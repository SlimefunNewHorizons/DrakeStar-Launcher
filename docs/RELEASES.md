# Canales y releases

| Canal | Significado | Riesgo |
|---|---|---|
| stable | verificada según los criterios del proyecto | 0 |
| unstable | integración; funciona pero sin garantías | 1 |
| nightly | experimental/automática | 2 |

**Regla de resolución** (`Channel::accepts`, `Manifest::resolve`): un perfil acepta releases de su canal o de uno **más estable**; nunca de uno más arriesgado. Además la release debe declarar la versión de Minecraft del perfil y un artefacto para la plataforma (o `any`). Gana la versión numérica más alta.

**Manifiesto** ([schema](../schemas/release-manifest.schema.json)): releases → artefactos `{component, platform, url, sha256, size}`. Componentes previstos: `launcher`, `drakes-rust-client`, `drakespurpur`, `drakes-natives`. El CLI rechaza artefactos sin SHA-256 válido (`drakestar manifest-check`).

**Visibilidad en la UI:** cada perfil muestra su pastilla de canal (`assets/brand/ui/status-channel-*.svg`); cambiar de canal exige confirmación.

**Pendiente:** firma del manifiesto, rollback automático. (La **publicación por CI** ya existe: workflow `release.yml` que compila, firma y sube `latest.json` — ver [PUBLICAR.md](PUBLICAR.md#publicar-una-versión-con-el-ci-no-compiles-a-mano).)
