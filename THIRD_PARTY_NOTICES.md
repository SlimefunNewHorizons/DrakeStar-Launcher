# Third-party notices

## Starling / Horizon Android (ideas de interfaz)

- Proyecto: https://github.com/starling-client/starling
- Referencia: [Starling v0.8.0](https://github.com/starling-client/starling/releases/tag/v0.8.0).
- Licencia upstream: [MIT, en el tag consultado](https://github.com/starling-client/starling/blob/v0.8.0/LICENSE).
- Catálogo, perfiles Ely.by, Diagnóstico y acceso a carpetas se implementaron de forma independiente sobre DrakeStar. No se copió código, recursos ni binarios de Starling/Horizon.
- Cualquier futura incorporación de código debe conservar íntegros la licencia MIT y los avisos de copyright aplicables del archivo/tag original en esta distribución. Esta referencia a ideas no sustituye esos avisos si se copia código.

## authlib-injector (dependencia descargada al usar Ely.by)

- Proyecto y código fuente de la versión: https://github.com/yushijinhun/authlib-injector/tree/v1.2.8
- Autor: yushijinhun y colaboradores.
- Licencia: GNU Affero General Public License v3 o posterior con la excepción AUTHLIB-INJECTOR. Texto completo: https://github.com/yushijinhun/authlib-injector/blob/v1.2.8/AGPLv3-with-authlib-injector-exception.txt
- [Aviso de licencia y excepción](https://github.com/yushijinhun/authlib-injector/blob/v1.2.8/COPYING.md).
- El launcher descarga sin modificar el JAR oficial 1.2.8 al preparar una sesión Ely.by; no se incluye en sus instaladores. Verifica SHA-256 `9c7f4343e6c82034958ffb48c14a2cb0c85928be7283103ce17da00c6d5a7b10` antes de ejecutarlo como Java Agent. No se copió código fuente de authlib-injector en DrakeStar.

## Contenido del catálogo Modrinth

El launcher consulta la API pública de Modrinth y descarga archivos desde su CDN. Cada mod, resource pack y shader conserva la licencia y los créditos de su autor. No se incluyen estos archivos en los instaladores del launcher.
