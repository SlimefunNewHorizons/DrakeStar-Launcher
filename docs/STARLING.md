# Funciones inspiradas en Starling v0.8.0

Implementación independiente; fuentes y avisos en [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md).

- [x] Catálogo Modrinth en la app: buscar por versión de instancia, instalar y quitar mods Fabric, resource packs y shaders.
- [x] Perfiles Ely.by junto a Microsoft y sin premium: login/2FA, token separado y renovación, javaagent oficial verificado.
- [x] Diagnóstico: fallo/etapa/instancia, salida de sesión y registros, reporte y regreso a Jugar/editar/cuentas.
- [x] Abrir carpeta de instancia desde Jugar, Instancias, Catálogo y el diagnóstico de fallo; los errores del explorador se muestran.
- [ ] Validación manual en escritorio: cuenta Ely.by real con y sin 2FA, arranque del juego, carpetas en Windows/Linux/macOS. No se dispone de una cuenta de prueba ni de sesión gráfica en el VPS.

## Uso y límites

En **Catálogo**, elige instancia y contenido, busca e instala. Se instalan releases compatibles y sus dependencias requeridas, incluidas las versiones exactas indicadas por Modrinth. Shaders requiere Fabric y añade Iris; se activa en el menú de vídeo del juego. Los resource packs se activan en el menú de recursos. Vanilla admite resource packs.

El catálogo registra los proyectos en `.drakestar-catalog.json` dentro de la instancia. Solo retira archivos propios cuyo SHA-512 sigue coincidiendo. Un archivo idéntico que ya existía se registra como externo y se conserva al quitar el proyecto. No elimina archivos por prefijos. Antes de quitar una dependencia, exige quitar los proyectos que la usan. Las dependencias quedan en la lista hasta retirarlas expresamente. No actualiza automáticamente contenido ya registrado. Si cambia la versión de Minecraft, retira el contenido incompatible antes de reinstalarlo.

La descarga está limitada a 256 MiB por archivo, exclusivamente desde el CDN HTTPS de Modrinth, sin extracción. Rutas, nombres y enlaces simbólicos se validan antes de escribir. Una descarga fallida no elimina archivos anteriores; las dependencias ya instaladas permanecen registradas y se pueden quitar. No es una transacción del conjunto completo. No modifica contenido mientras se prepara o ejecuta una partida. El modo Rendimiento continúa restaurando sus mods al jugar mientras siga activado.

En **Cuentas → Ely.by**, introduce usuario, contraseña y opcionalmente seis dígitos 2FA. Contraseña y código no se persisten ni se registran; se vacían tras el intento. Solo se guarda el token de sesión y su client token en `accounts/secrets.json`, fuera de los datos públicos; permisos Unix 0600 y directorio 0700. En Windows se usa el perfil del usuario, como para Microsoft. El token se renueva antes de jugar. `authlib-injector` se descarga de su release oficial fijado, se verifica SHA-256 y se añade únicamente a la JVM del cliente Ely.by. Una sesión Ely.by no autentica contra servidores exclusivamente Microsoft: requiere servidores que acepten Ely.by o sin premium. No cambia la autenticación del servidor.

En **Diagnóstico**, Launcher muestra la preparación y Juego muestra stdout/stderr de esta sesión, incluso si el cliente falla antes de crear `latest.log`. En una nueva sesión de la app se usa el archivo `launcher-session.log` si existe, y `latest.log` como respaldo. El fallo conserva la instancia del intento para mostrar registros y generar reportes aunque se cambie la selección. Al volver a Jugar se puede corregir la instancia/cuenta y reintentar. Al cambiar de instancia o pestaña, una respuesta retrasada de la consulta anterior no sustituye los registros actuales.

## Conservación ante errores de persistencia

Las mutaciones de cuentas rechazan JSON ilegible o dañado antes de escribir.
Si falla la escritura del listado después de guardar credenciales, se restauran
los bytes anteriores de `secrets.json` (o se retira el archivo nuevo), con permisos
0600 en Unix. También se aplica a Microsoft y a la retirada de cuentas. Si la
restauración falla, se indica conservar `accounts/` para recuperar los datos.
Esto cubre errores de E/S durante la operación; los dos archivos no forman una
transacción resistente al cierre abrupto del proceso ni a escritores simultáneos.

El catálogo guarda el índice antes de borrar el archivo propio. Si falla el
índice, el contenido permanece; si falla el borrado, se restaura la entrada.
Un cierre abrupto entre ambos pasos puede dejar un archivo conservado fuera del
índice; debe revisarse en la carpeta de la instancia. La preparación conserva el
tipo de cuenta elegido para añadir el agente Ely.by aunque se retire el perfil
durante las descargas. Este último caso requiere todavía ensayo gráfico.

## Validación reproducible

Un build a la vez, desde Star:

```sh
~/ai-hub/scripts/build_seguro.sh 21 "$PWD" ~/.cargo/bin/cargo test -p drakestar-mc -j 2
~/ai-hub/scripts/build_seguro.sh 21 "$PWD/ui" npm run build
~/ai-hub/scripts/build_seguro.sh 21 "$PWD" env DRAKESTAR_CATALOG_SMOKE=1 ~/.cargo/bin/cargo run -p drakestar-mc --example catalog_smoke -j 2
```

El smoke hace búsquedas reales, instala/quita FerriteCore, Bare Bones y Complementary Reimagined (Iris/Sodium incluidos) para Minecraft 1.21.1 en una carpeta temporal; no abre Minecraft, no usa cuentas ni instancias del jugador, y limpia el destino. La capa Tauri se valida en CI, porque Star no tiene `webkit2gtk-4.1` de desarrollo.

## Android: evaluación separada, sin promesa de compatibilidad

[Starling v0.8.0](https://github.com/starling-client/starling/releases/tag/v0.8.0) es un cliente Android de Horizon para Java 26.3; no es un launcher JVM ni ejecuta los mods Fabric del catálogo de DrakeStar. El [README de ese tag](https://github.com/starling-client/starling/blob/v0.8.0/README.md) exige recursos de Minecraft Java 26.3 propios. La licencia MIT cubre Starling, no los recursos de Mojang.

Dallas permanece en 26.2 según la orden de Jack. Esta entrega no ha consultado ni modificado Dallas. No hay evidencia de que Starling pueda entrar en DrakesCraft.

Antes de ofrecerlo a jugadores:

1. Preparar un servidor de laboratorio 26.2 con la misma configuración relevante y ViaVersion/ViaBackwards compatibles. La traducción de un cliente más nuevo requiere soporte explícito del protocolo 26.3; tener los plugins instalados por sí solo no lo demuestra.
2. En un Android real con APK 0.8.0, probar conexión, identidad/login, entrada al mundo, inventarios y contenedores, resource pack, cabezas Slimefun, chat y reconexión. Guardar versiones exactas y logs.
3. Comparar con Java 26.2 en el mismo laboratorio. Comprobar anti-cheat, mecánicas y plugins que dependan de paquetes. No habilitarlo en Dallas ni anunciar compatibilidad hasta obtener evidencia.

- [x] Evaluación y condiciones de prueba documentadas.
- [ ] Prueba Android 26.3 ↔ laboratorio 26.2 con ViaVersion/ViaBackwards y resultados.
