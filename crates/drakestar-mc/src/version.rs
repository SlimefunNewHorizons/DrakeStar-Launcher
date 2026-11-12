//! JSON de version de Mojang (y de Fabric, que usa el mismo formato con `inheritsFrom`): tipos, reglas y fusion.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionJson {
    pub id: String,
    pub inherits_from: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub main_class: Option<String>,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub arguments: Option<Arguments>,
    /// Formato antiguo (<1.13): una sola cadena.
    pub minecraft_arguments: Option<String>,
    pub libraries: Vec<Library>,
    pub downloads: Option<Downloads>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<Arg>,
    pub jvm: Vec<Arg>,
}

/// Un argumento: texto simple o condicionado por reglas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub rules: Option<Vec<Rule>>,
    /// Natives de formato antiguo: `{"windows": "natives-windows"}`.
    pub natives: Option<BTreeMap<String, String>>,
    pub extract: Option<Extract>,
    /// Fabric: URL base de maven (sin hash).
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Extract {
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<BTreeMap<String, Artifact>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Downloads {
    pub client: Option<FileRef>,
    pub server: Option<FileRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileRef {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingClient {
    pub argument: String,
    pub file: Option<LoggingFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Contexto contra el que se evaluan las reglas.
#[derive(Debug, Clone)]
pub struct RuleCtx {
    pub os: String,
    pub arch: String,
    pub features: BTreeSet<String>,
}

impl RuleCtx {
    pub fn current(features: &[&str]) -> Self {
        Self {
            os: crate::os_name().into(),
            arch: crate::arch_name().into(),
            features: features.iter().map(|s| s.to_string()).collect(),
        }
    }
}

fn rule_matches(r: &Rule, ctx: &RuleCtx) -> bool {
    if let Some(os) = &r.os {
        if let Some(n) = &os.name {
            if n != &ctx.os {
                return false;
            }
        }
        if let Some(a) = &os.arch {
            // Mojang usa "x86" para 32 bits y a veces omite el campo en 64 bits
            if a != &ctx.arch {
                return false;
            }
        }
    }
    if let Some(f) = &r.features {
        for (k, want) in f {
            if ctx.features.contains(k) != *want {
                return false;
            }
        }
    }
    true
}

/// Reglas de Mojang: sin reglas = permitido; el ultimo `rule` que coincide decide.
pub fn allowed(rules: &Option<Vec<Rule>>, ctx: &RuleCtx) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut ok = false;
    for r in rules {
        if rule_matches(r, ctx) {
            ok = r.action == "allow";
        }
    }
    ok
}

pub fn args_allowed(rules: &[Rule], ctx: &RuleCtx) -> bool {
    allowed(&Some(rules.to_vec()), ctx)
}

impl Arg {
    /// Valores del argumento si pasa las reglas.
    pub fn values(&self, ctx: &RuleCtx) -> Vec<String> {
        match self {
            Arg::Plain(s) => vec![s.clone()],
            Arg::Ruled { rules, value } if args_allowed(rules, ctx) => match value {
                ArgValue::One(s) => vec![s.clone()],
                ArgValue::Many(v) => v.clone(),
            },
            _ => vec![],
        }
    }
}

impl VersionJson {
    pub fn parse(text: &str) -> Result<Self, crate::McError> {
        Ok(serde_json::from_str(text)?)
    }

    /// Une un JSON hijo (Fabric) con su padre (vanilla): el hijo manda en `mainClass`, sus librerias van primero
    /// en el classpath y sus argumentos se anaden a los del padre.
    pub fn merged(parent: &VersionJson, child: &VersionJson) -> VersionJson {
        let mut libs = child.libraries.clone();
        libs.extend(parent.libraries.iter().cloned());
        let mut args = parent.arguments.clone().unwrap_or_default();
        if let Some(c) = &child.arguments {
            args.game.extend(c.game.iter().cloned());
            args.jvm.extend(c.jvm.iter().cloned());
        }
        VersionJson {
            id: child.id.clone(),
            inherits_from: child.inherits_from.clone(),
            kind: child.kind.clone().or_else(|| parent.kind.clone()),
            main_class: child.main_class.clone().or_else(|| parent.main_class.clone()),
            assets: child.assets.clone().or_else(|| parent.assets.clone()),
            asset_index: child.asset_index.clone().or_else(|| parent.asset_index.clone()),
            arguments: if parent.arguments.is_some() || child.arguments.is_some() { Some(args) } else { None },
            minecraft_arguments: child.minecraft_arguments.clone().or_else(|| parent.minecraft_arguments.clone()),
            libraries: libs,
            downloads: child.downloads.clone().or_else(|| parent.downloads.clone()),
            java_version: child.java_version.clone().or_else(|| parent.java_version.clone()),
            logging: child.logging.clone().or_else(|| parent.logging.clone()),
        }
    }
}

/// `group:artifact:version[:classifier]` -> ruta maven (`group/artifact/version/artifact-version[-classifier].jar`).
pub fn maven_path(name: &str) -> Option<String> {
    let (coord, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (g, a, v) = (parts[0].replace('.', "/"), parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(c) => format!("{a}-{v}-{c}.{ext}"),
        None => format!("{a}-{v}.{ext}"),
    };
    Some(format!("{g}/{a}/{v}/{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(), "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert!(maven_path("mal").is_none());
    }

    #[test]
    fn rules_last_match_wins() {
        let ctx = RuleCtx { os: "windows".into(), arch: "x86_64".into(), features: BTreeSet::new() };
        let allow = Rule { action: "allow".into(), ..Default::default() };
        let no_osx = Rule { action: "disallow".into(), os: Some(OsRule { name: Some("osx".into()), arch: None }), features: None };
        assert!(allowed(&Some(vec![allow.clone(), no_osx.clone()]), &ctx));
        let osx_ctx = RuleCtx { os: "osx".into(), ..ctx.clone() };
        assert!(!allowed(&Some(vec![allow, no_osx]), &osx_ctx));
        assert!(allowed(&None, &ctx));
    }

    #[test]
    fn features_gate_arguments() {
        let ctx = RuleCtx::current(&["has_custom_resolution"]);
        let a: Arg = serde_json::from_str(r#"{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]}"#).unwrap();
        assert_eq!(a.values(&ctx).len(), 2);
        let none = RuleCtx::current(&[]);
        assert!(a.values(&none).is_empty());
    }

    #[test]
    fn merge_puts_child_libs_first() {
        let p = VersionJson { id: "1.21.1".into(), main_class: Some("net.minecraft.client.main.Main".into()), libraries: vec![Library { name: "a:b:1".into(), ..Default::default() }], ..Default::default() };
        let c = VersionJson { id: "fabric".into(), inherits_from: Some("1.21.1".into()), main_class: Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into()), libraries: vec![Library { name: "c:d:2".into(), ..Default::default() }], ..Default::default() };
        let m = VersionJson::merged(&p, &c);
        assert_eq!(m.libraries[0].name, "c:d:2");
        assert_eq!(m.main_class.as_deref(), Some("net.fabricmc.loader.impl.launch.knot.KnotClient"));
    }
}
