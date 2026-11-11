//! Clientes Java gestionados (Fabric). Capa PURA: lee los JSON oficiales de
//! Mojang/Fabric y produce el plan de lanzamiento; no descarga ni ejecuta nada
//! (eso es `drakestar-net` y `drakestar-app`).
//!
//! shortcut: sin cuenta Microsoft: se lanza en modo offline (`--accessToken 0`),
//! que sirve para singleplayer y servidores `online-mode=false`. Para entrar a
//! servidores online hace falta OAuth de Microsoft (guarda refresh tokens):
//! pendiente, y hasta entonces la UI no debe ofrecerlo como si funcionara.

use crate::profile::{Loader, Mode, Profile};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Main class que inyecta Fabric Loader sobre el cliente de Mojang.
pub const KNOT_MAIN_CLASS: &str = "net.fabricmc.loader.impl.launch.knot.KnotClient";

/// UUID vacio: en modo offline el servidor calcula el suyo a partir del nombre.
pub const OFFLINE_UUID: &str = "00000000-0000-0000-0000-000000000000";

// ---------- manifiesto de versiones de Mojang ----------

#[derive(Debug, Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
}

/// La version que pide el perfil tiene que estar publicada tal cual.
pub fn pick_version<'a>(entries: &'a [VersionEntry], want: &str) -> Option<&'a VersionEntry> {
    entries.iter().find(|e| e.id == want)
}

// ---------- JSON de una version concreta ----------

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub downloads: VersionDownloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndex>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersionReq>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
}

#[derive(Debug, Deserialize)]
pub struct VersionDownloads {
    pub client: Artifact,
}

#[derive(Debug, Deserialize)]
pub struct JavaVersionReq {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Un artefacto descargable: ruta maven (si el JSON la da), hash, tamano y URL.
#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    /// El `client` del manifiesto no la trae: se usa `client.jar`.
    #[serde(default)]
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Clasificadores de nativos por sistema ("linux" -> "natives-linux").
    #[serde(default)]
    pub natives: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<BTreeMap<String, Artifact>>,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub action: Action,
    #[serde(default)]
    pub os: Option<OsRule>,
    /// `features` (demo, quickPlay, resolucion custom): no soportados, y una
    /// regla que los pide nunca aplica.
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Debug, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    /// Mojang solo la usa con "x86" (32 bits) para flags de ese sistema.
    #[serde(default)]
    pub arch: Option<String>,
}

/// Sistema operativo para las reglas y los clasificadores de nativos.
pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Arquitectura tal como la nombra Mojang (`std::env::consts::ARCH`).
pub fn arch_name() -> &'static str {
    std::env::consts::ARCH
}

/// Una regla aplica aqui si su `os` (nombre y arquitectura) es el nuestro y no
/// pide `features`. La primera regla que aplica decide; si ninguna aplica, se
/// usa el default: las librerías se permiten (Mojang lo hace asi), los
/// argumentos se descartan.
fn eval_rules(rules: &[Rule], default: bool) -> bool {
    for r in rules {
        let os = r.os.as_ref();
        let applies = r.features.is_empty()
            && os.is_none_or(|o| {
                o.name.as_deref().is_none_or(|n| n == os_name())
                    && o.arch.as_deref().is_none_or(|a| a == arch_name())
            });
        if applies {
            return r.action == Action::Allow;
        }
    }
    default
}

fn rule_permits(rules: &[Rule]) -> bool {
    eval_rules(rules, true)
}

/// Artefactos de una libreria para este sistema: el jar (si tiene) y el nativo
/// (si la libreria trae clasificadores para aqui). `None` si una regla la veda.
pub fn select(lib: &Library) -> Option<(Option<&Artifact>, Option<&Artifact>)> {
    if !rule_permits(&lib.rules) {
        return None;
    }
    let d = lib.downloads.as_ref()?;
    let native = lib
        .natives
        .as_ref()
        .and_then(|n| n.get(os_name()))
        .and_then(|k| d.classifiers.as_ref().and_then(|c| c.get(k)));
    Some((d.artifact.as_ref(), native))
}

// ---------- argumentos del manifiesto ----------

/// Valor de un argumento gobernado por reglas: uno o varios.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

/// Entrada de `arguments.jvm` / `arguments.game`: texto suelto o texto con reglas.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled(RuledArg),
}

#[derive(Debug, Deserialize)]
pub struct RuledArg {
    pub rules: Vec<Rule>,
    pub value: ArgValue,
}

/// Argumento gobernado por reglas: se incluye si no trae reglas o si alguna le
/// aplica y permite. Si trae reglas y ninguna aplica (p. ej. un flag solo de
/// Windows), se descarta.
fn arg_included(rules: &[Rule]) -> bool {
    rules.is_empty() || eval_rules(rules, false)
}

/// Argumentos de JVM/ juego que aplican a este sistema, con los placeholders
/// sustituidos. Un placeholder desconocido descarta el argumento entero.
fn expand_args(args: &[Arg], vars: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    fn emit(v: &ArgValue, vars: &dyn Fn(&str) -> Option<String>, out: &mut Vec<String>) {
        match v {
            ArgValue::One(s) => {
                if let Some(x) = fill(s, vars) {
                    out.push(x);
                }
            }
            ArgValue::Many(xs) => {
                for s in xs {
                    if let Some(x) = fill(s, vars) {
                        out.push(x);
                    }
                }
            }
        }
    }
    let mut out = vec![];
    for a in args {
        match a {
            Arg::Plain(s) => {
                if let Some(x) = fill(s, vars) {
                    out.push(x);
                }
            }
            Arg::Ruled(r) => {
                if arg_included(&r.rules) {
                    emit(&r.value, vars, &mut out);
                }
            }
        }
    }
    out
}

/// Sustituye `${clave}`; devuelve `None` si alguna clave es desconocida.
fn fill(s: &str, vars: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let end = after.find('}')?;
        let key = &after[..end];
        out.push_str(&vars(key)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

#[derive(Debug, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub jvm: Vec<Arg>,
    #[serde(default)]
    pub game: Vec<Arg>,
}

// ---------- plan de lanzamiento ----------

/// El plan del cliente es el mismo `LaunchPlan` que usa el servidor local.
pub type ClientPlan = crate::launch::LaunchPlan;

/// Lo que concreta el lanzador al armar la linea: rutas ya resueltas por
/// `drakestar-app` y la identidad offline.
pub struct ClientLaunch<'a> {
    pub java_bin: &'a str,
    pub username: &'a str,
    /// jars ya descargados y verificados, separados por `:`.
    pub classpath: &'a str,
    pub game_dir: &'a str,
    pub assets_dir: &'a str,
    pub natives_dir: &'a str,
    /// etiqueta de version que se ve en el juego (p. ej. "fabric-loader-0.20.1-26.2").
    pub version_label: &'a str,
    pub launcher_name: &'a str,
    pub launcher_version: &'a str,
    /// UUID con guiones o en formato Mojang (si hay sesión online).
    pub uuid: Option<&'a str>,
    /// Access token oficial de Minecraft (si hay sesión online).
    pub access_token: Option<&'a str>,
    /// Tipo de usuario: "msa" (Microsoft) o "mojang".
    pub user_type: Option<&'a str>,
}

/// Plan de un cliente Fabric: los argumentos de JVM y de juego del manifiesto de
/// Mojang, filtrados por sistema y con los placeholders sustituidos, mas la
/// memoria del perfil. `None` si el perfil no es `drakes-java` con Fabric.
pub fn client_plan(p: &Profile, v: &VersionInfo, c: &ClientLaunch) -> Option<ClientPlan> {
    if p.mode != Mode::DrakesJava || p.loader != Loader::Fabric {
        return None;
    }
    let default_uuid = OFFLINE_UUID.to_string();
    let auth_uuid = c.uuid.unwrap_or(&default_uuid);
    let default_token = "0";
    let auth_token = c.access_token.unwrap_or(default_token);
    let default_user_type = "msa";
    let auth_user_type = c.user_type.unwrap_or(default_user_type);

    let vars = |key: &str| -> Option<String> {
        Some(match key {
            "natives_directory" => c.natives_dir.to_string(),
            "classpath" => c.classpath.to_string(),
            "launcher_name" => c.launcher_name.to_string(),
            "launcher_version" => c.launcher_version.to_string(),
            "auth_player_name" => c.username.to_string(),
            "version_name" => c.version_label.to_string(),
            "game_directory" => c.game_dir.to_string(),
            "assets_root" => c.assets_dir.to_string(),
            "assets_index_name" => v.asset_index.as_ref().map(|a| a.id.clone())?,
            "auth_uuid" => auth_uuid.to_string(),
            "auth_access_token" => auth_token.to_string(),
            "user_type" => auth_user_type.to_string(),
            "clientid" => String::new(),
            "auth_xuid" => String::new(),
            "version_type" => "release".to_string(),
            _ => return None,
        })
    };
    let mut args: Vec<String> = vec![
        format!("-Xms{}M", p.java.min_mem_mb),
        format!("-Xmx{}M", p.java.max_mem_mb),
    ];
    args.extend(p.java.extra_jvm_args.iter().cloned());
    if let Some(a) = &v.arguments {
        args.extend(expand_args(&a.jvm, &vars));
    }
    args.push(KNOT_MAIN_CLASS.to_string());
    if let Some(a) = &v.arguments {
        args.extend(expand_args(&a.game, &vars));
    }
    Some(ClientPlan { program: c.java_bin.into(), args, cwd: c.game_dir.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{profile::JavaSettings, Channel};

    fn base(mode: Mode, loader: Loader) -> Profile {
        Profile {
            schema: 1,
            id: "fab".into(),
            name: "Fabric".into(),
            mode,
            channel: Channel::Stable,
            minecraft: "26.2".into(),
            loader,
            java: JavaSettings::default(),
            server: None,
            preset: None,
            modpack: None,
            notes: String::new(),
        }
    }

    #[test]
    fn pick_version_is_exact() {
        let m: VersionManifest =
            serde_json::from_str(r#"{"versions":[{"id":"26.1","url":"https://a/26.1.json"},{"id":"26.2","url":"https://a/26.2.json"}]}"#)
                .unwrap();
        assert_eq!(pick_version(&m.versions, "26.2").unwrap().url, "https://a/26.2.json");
        assert!(pick_version(&m.versions, "26.3").is_none());
    }

    #[test]
    fn version_json_parses_and_selects_for_this_os() {
        // libreria comun + libreria vetada en linux + libreria de nativos
        let v: VersionInfo = serde_json::from_str(
            r#"{
              "id": "26.2",
              "mainClass": "net.minecraft.client.main.Main",
              "assetIndex": { "id": "26.2", "sha1": "abc", "size": 1, "url": "https://a/index.json" },
              "javaVersion": { "component": "java-runtime-delta", "majorVersion": 25 },
              "downloads": { "client": { "path": "c.jar", "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 10, "url": "https://a/c.jar" } },
              "libraries": [
                { "name": "com.mojang:logging:1.0.0",
                  "downloads": { "artifact": { "path": "com/mojang/logging/1.0.0/logging-1.0.0.jar", "sha1": "b", "size": 2, "url": "https://a/l.jar" } } },
                { "name": "org.example:only-mac:1", "rules": [ { "action": "allow", "os": { "name": "osx" } }, { "action": "disallow" } ],
                  "downloads": { "artifact": { "path": "m.jar", "sha1": "c", "size": 1, "url": "https://a/m.jar" } } },
                { "name": "org.lwjgl:lwjgl:3.3.1", "natives": { "linux": "natives-linux" },
                  "downloads": { "artifact": { "path": "lwjgl.jar", "sha1": "d", "size": 1, "url": "https://a/lwjgl.jar" },
                                 "classifiers": { "natives-linux": { "path": "lwjgl-natives-linux.jar", "sha1": "e", "size": 1, "url": "https://a/n.jar" } } } }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(v.main_class, "net.minecraft.client.main.Main");
        assert_eq!(v.java_version.unwrap().major_version, 25);
        assert_eq!(v.asset_index.unwrap().id, "26.2");
        let [common, mac, lwjgl] = &v.libraries[..] else { panic!("3 librerias") };
        assert!(select(common).unwrap().0.is_some());
        // solo-mac: en macOS se permite, en el resto se veta (Mojang publica el
        // disallow explicito; sin el, la regla que no aplica no veta nada)
        assert_eq!(select(mac).is_some(), os_name() == "osx");
        let (jar, native) = select(lwjgl).unwrap();
        assert_eq!(jar.unwrap().path, "lwjgl.jar");
        if os_name() == "linux" {
            assert_eq!(native.unwrap().path, "lwjgl-natives-linux.jar");
        }
    }

    #[test]
    fn rules_macos_only_are_ignored_on_linux() {
        let lib: Library = serde_json::from_str(
            r#"{"name":"x","rules":[{"action":"allow","os":{"name":"osx"}},{"action":"disallow"}],
                "downloads":{"artifact":{"path":"p","sha1":"s","size":1,"url":"u"}}}"#,
        )
        .unwrap();
        assert_eq!(select(&lib).is_some(), os_name() == "osx");
    }

    #[test]
    fn first_matching_rule_decides() {
        let lib: Library = serde_json::from_str(
            r#"{"name":"x","rules":[{"action":"allow"},{"action":"disallow","os":{"name":"linux"}}],
                "downloads":{"artifact":{"path":"p","sha1":"s","size":1,"url":"u"}}}"#,
        )
        .unwrap();
        // la primera regla no tiene os: aplica siempre y permite
        assert!(select(&lib).is_some());
    }

    fn sample_version() -> VersionInfo {
        serde_json::from_str(
            r#"{
              "id": "26.2",
              "mainClass": "net.minecraft.client.main.Main",
              "assetIndex": { "id": "32", "sha1": "abc", "size": 1, "url": "https://a/index.json" },
              "javaVersion": { "component": "java-runtime-epsilon", "majorVersion": 25 },
              "downloads": { "client": { "path": "c.jar", "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 10, "url": "https://a/c.jar" } },
              "arguments": {
                "jvm": [
                  { "rules": [ { "action": "allow", "os": { "name": "osx" } } ], "value": [ "-XstartOnFirstThread" ] },
                  { "rules": [ { "action": "allow", "os": { "name": "windows" } } ], "value": "-XX:HeapDumpPath=x" },
                  { "rules": [ { "action": "allow", "os": { "arch": "x86" } } ], "value": "-Xss1M" },
                  { "rules": [ { "action": "allow", "features": { "is_demo_user": true } } ], "value": "--demo" },
                  "--enable-native-access=ALL-UNNAMED",
                  "-Djava.library.path=${natives_directory}/java",
                  "-Dminecraft.launcher.brand=${launcher_name}",
                  "-cp", "${classpath}"
                ],
                "game": [ "--username", "${auth_player_name}", "--version", "${version_name}",
                          "--gameDir", "${game_directory}", "--assetsDir", "${assets_root}",
                          "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                          "--accessToken", "${auth_access_token}", "--versionType", "${version_type}" ]
              },
              "libraries": []
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn client_plan_only_for_fabric_and_carries_offline_identity() {
        let p = base(Mode::DrakesJava, Loader::Fabric);
        let v = sample_version();
        let l = ClientLaunch {
            java_bin: "java",
            username: "nix",
            classpath: "a.jar:b.jar",
            game_dir: "/d/inst",
            assets_dir: "/d/assets",
            natives_dir: "/d/natives",
            version_label: "fabric-loader-0.20.1-26.2",
            launcher_name: "DrakeStar",
            launcher_version: "0.1.0",
            uuid: None,
            access_token: None,
            user_type: None,
        };
        let plan = client_plan(&p, &v, &l).unwrap();
        assert_eq!(plan.cwd, "/d/inst");
        assert_eq!(plan.program, "java");
        // la memoria del perfil va primero y el classpath es el del manifiesto
        assert!(plan.args.contains(&"-Xmx4096M".to_string()));
        assert!(plan.args.windows(2).any(|w| w == ["-cp", "a.jar:b.jar"]));
        assert!(plan.args.contains(&KNOT_MAIN_CLASS.to_string()));
        assert!(plan.args.windows(2).any(|w| w == ["--username", "nix"]));
        assert!(plan.args.windows(2).any(|w| w == ["--accessToken", "0"]));
        assert!(plan.args.windows(2).any(|w| w == ["--uuid", OFFLINE_UUID]));
        assert!(plan.args.windows(2).any(|w| w == ["--version", "fabric-loader-0.20.1-26.2"]));
        assert!(plan.args.contains(&"-Djava.library.path=/d/natives/java".to_string()));
        assert!(plan.args.contains(&"-Dminecraft.launcher.brand=DrakeStar".to_string()));
        // flags de sistema: solo los nuestros
        assert!(plan.args.contains(&"--enable-native-access=ALL-UNNAMED".to_string()));
        assert!(!plan.args.iter().any(|a| a.contains("HeapDumpPath")));
        assert!(!plan.args.iter().any(|a| a.contains("startOnFirstThread") || os_name() == "osx"));
        // demo, 32 bits y placeholders desconocidos: fuera
        assert!(!plan.args.contains(&"--demo".to_string()));
        assert!(!plan.args.contains(&"-Xss1M".to_string()));
        assert!(!plan.args.iter().any(|a| a.contains("${")));

        // vanilla/forge/neoforge y el resto de modos no generan plan de cliente
        for ldr in [Loader::Vanilla, Loader::Forge, Loader::Neoforge] {
            assert!(client_plan(&base(Mode::DrakesJava, ldr), &v, &l).is_none());
        }
        for m in [Mode::LocalServer, Mode::Online, Mode::DrakesRust] {
            assert!(client_plan(&base(m, Loader::Fabric), &v, &l).is_none());
        }
    }
}
