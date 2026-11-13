//! Prueba opt-in contra Modrinth, en un directorio temporal. Nunca usa instancias del jugador.
use drakestar_mc::{
    catalog::{self, Kind},
    instances::Instance,
    Dirs,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("DRAKESTAR_CATALOG_SMOKE").as_deref() != Ok("1") {
        return Err("Activa DRAKESTAR_CATALOG_SMOKE=1 para descargar contenido de prueba".into());
    }
    let root = std::env::temp_dir().join(format!("drakestar-catalog-smoke-{}", std::process::id()));
    if root.exists() {
        return Err("El directorio temporal ya existe".into());
    }
    let d = Dirs::new(&root);
    let inst: Instance = serde_json::from_value(
        serde_json::json!({"id":"smoke", "name":"Smoke", "mc":"1.21.1", "loader":"fabric", "target":{"kind":"menu"}}),
    )?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for (kind, query, slug) in [
            (Kind::Mod, "ferritecore", "ferrite-core"),
            (Kind::Resourcepack, "Bare Bones", "bare-bones"),
            (
                Kind::Shader,
                "Complementary Reimagined",
                "complementary-reimagined",
            ),
        ] {
            let hits = catalog::search(&inst, kind, query)?;
            assert!(!hits.is_empty(), "{kind:?}: búsqueda sin resultados");
            let hit = hits
                .iter()
                .find(|h| h.slug == slug)
                .ok_or("Proyecto esperado ausente de la búsqueda")?;
            let entries = catalog::install(&d, &inst, kind, &hit.project_id)?;
            assert!(entries.iter().any(|e| e.project_id == hit.project_id));
            println!(
                "{kind:?}: búsqueda, descarga SHA-512 e instalación correctas ({} proyectos)",
                entries.len()
            );
            catalog::remove(&d, &inst, &hit.project_id)?;
        }
        // Retirar las dependencias de Iris en orden permitido por el grafo.
        loop {
            let entries = catalog::list(&d, &inst)?;
            if entries.is_empty() {
                break;
            }
            let leaf = entries
                .iter()
                .find(|e| {
                    !entries
                        .iter()
                        .any(|other| other.dependencies.contains(&e.project_id))
                })
                .ok_or("Ciclo inesperado")?;
            catalog::remove(&d, &inst, &leaf.project_id)?;
        }
        println!("Retirada completa: sin proyectos registrados");
        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    result
}
