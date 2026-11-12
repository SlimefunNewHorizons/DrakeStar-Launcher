// Textos de la interfaz en espanol (por defecto) e ingles. `t('clave', {var: valor})` es reactivo al cambiar de idioma.
const es: Record<string, string> = {
  'import.title': 'Importar instancia', 'import.path': 'Ruta de la carpeta o del archivo',
  'import.hint': 'Prism: carpeta o ZIP exportado. CurseForge: carpeta de la instancia instalada. Modrinth: archivo .mrpack exportado. Solo Vanilla y Fabric.',
  'import.safe': 'Copia mods, configuración y mundos a una instancia nueva. Conserva el origen y la versión de Fabric. No importa cuentas ni comandos de inicio. Límite: 4 GiB.',
  'import.preview': 'Revisar', 'import.copy': 'Importar copia', 'import.working': 'Copiando y descargando archivos…',
  'f.memory_hint': 'RAM del equipo: {total}. Recomendación: {recommended}, dejando memoria para el sistema y Java.',
  'f.memory_apply': 'Usar memoria recomendada',

  'nav.play': 'Jugar', 'nav.accounts': 'Cuentas', 'nav.instances': 'Instancias', 'nav.settings': 'Ajustes', 'nav.logs': 'Registros',
  'common.cancel': 'Cancelar', 'common.save': 'Guardar', 'common.close': 'Cerrar', 'common.copy': 'Copiar', 'common.copied': '¡Copiado!',
  'common.retry': 'Reintentar', 'common.open_folder': 'Abrir carpeta', 'common.advanced': 'Avanzado', 'common.yes': 'Sí', 'common.no': 'No', 'common.sure': '¿Seguro?',

  'play.button': 'JUGAR', 'play.preparing': 'Preparando…', 'play.running': 'En juego', 'play.close_game': 'Cerrar el juego',
  'play.account': 'Jugando como', 'play.instances': 'Elige qué jugar', 'play.first_time': 'La primera vez descarga Java y los archivos del juego; puede tardar unos minutos.',
  'play.no_account_title': 'Elige cómo jugar', 'play.no_account_text': 'Necesitas una cuenta para entrar al juego.',
  'play.crashed': 'El juego se cerró con un error{code}.', 'play.error': 'No se pudo iniciar el juego.', 'play.exited': 'El juego se cerró.', 'play.cancelled': 'Cancelado.',
  'play.report_sent': 'Se envió un reporte automático para poder arreglarlo (datos personales tachados).', 'play.report_saved': 'Se guardó un reporte en tu PC.',
  'play.view_logs': 'Ver registros', 'play.disable_fused': 'Desactivar Fused', 'play.send_report': 'Enviar reporte', 'play.no_instances': 'No hay instancias. Crea una en la pestaña Instancias.',
  'target.menu': 'Menú principal', 'target.server': 'Servidor {addr}', 'target.local': 'Servidor propio en este equipo', 'target.world': 'Mundo «{name}»',

  'acc.title': 'Cuentas', 'acc.subtitle': 'Puedes tener varias cuentas, premium y sin premium, y elegir con cuál jugar.',
  'acc.add_ms': 'Iniciar sesión con Microsoft', 'acc.add_ms_hint': 'Cuenta premium de Minecraft Java. Se abre tu navegador solo.',
  'acc.add_off': 'Jugar sin premium', 'acc.add_off_hint': 'Para mundos en solitario/LAN y servidores que lo permitan.',
  'acc.name_ph': 'Nombre de jugador', 'acc.name_rule': '3 a 16 letras, números o _', 'acc.add': 'Agregar',
  'acc.default': 'Predeterminada', 'acc.use': 'Usar', 'acc.remove': 'Quitar', 'acc.confirm_remove': '¿Quitar la cuenta {name}? Podrás volver a agregarla.',
  'acc.microsoft': 'Microsoft', 'acc.offline': 'Sin premium', 'acc.empty': 'Aún no hay cuentas.',
  'acc.temp_id': 'El inicio de sesión de Microsoft funciona en modo temporal mientras Mojang aprueba la app de DrakeStar. Si falla, usa «Jugar sin premium».',
  'acc.invalid_name': 'Nombre no válido: usa 3 a 16 letras, números o _.',

  'msa.title': 'Iniciar sesión con Microsoft', 'msa.opening': 'Abriendo el navegador…',
  'msa.waiting': 'Se abrió tu navegador. Inicia sesión con tu cuenta y, si te lo pide, escribe este código:',
  'msa.reopen': 'Abrir la página otra vez', 'msa.working': 'Verificando tu cuenta de Minecraft…', 'msa.expires': 'El código caduca en {min}:{sec}', 'msa.failed': 'No se pudo iniciar sesión',

  'ins.title': 'Instancias', 'ins.subtitle': 'Cada instancia es una versión de Minecraft con sus mods y su carpeta de partidas.',
  'ins.new': 'Nueva instancia', 'ins.edit': 'Editar', 'ins.delete': 'Eliminar', 'ins.confirm_delete': '¿Eliminar la instancia «{name}»? Sus partidas guardadas no se borran del disco.',
  'ins.range': 'Compatible: Minecraft {min} a {max}.',
  'f.name': 'Nombre', 'f.version': 'Versión de Minecraft', 'f.loader': 'Tipo', 'f.vanilla': 'Vanilla (sin mods)', 'f.fabric': 'Fabric (con mods)',
  'f.target': 'Al darle a JUGAR', 'f.t_menu': 'Abrir el menú', 'f.t_server': 'Entrar a un servidor', 'f.t_world': 'Abrir un mundo',
  'f.address': 'Dirección del servidor', 'f.world': 'Nombre del mundo', 'f.perf': 'Mods de rendimiento', 'f.perf_hint': 'Sodium, Lithium, FerriteCore, ImmediatelyFast y Entity Culling: más FPS.',
  'f.fused': 'DrakeStar Fused (alfa)', 'f.fused_hint': 'Experimental: prepara Slimefun y plugins dentro de tu mundo en solitario/LAN, de Minecraft 1.20 a 26.2. Aún no ejecuta plugins.',
  'f.fabric_only': 'Solo con Fabric.', 'f.ram': 'Memoria (RAM)', 'f.jvm': 'Argumentos de Java', 'f.note': 'Nota', 'f.new_title': 'Nueva instancia', 'f.edit_title': 'Editar instancia',

  'set.title': 'Ajustes', 'set.updates': 'Actualizaciones', 'set.version': 'Versión {v}', 'set.check': 'Buscar actualizaciones', 'set.checking': 'Buscando…',
  'set.uptodate': 'Ya tienes la última versión.', 'set.check_failed': 'No se pudo comprobar si hay actualizaciones. Revisa tu conexión e inténtalo más tarde.', 'set.update_avail': 'Hay una versión nueva: {v}', 'set.install': 'Actualizar y reiniciar', 'set.installing': 'Instalando…',
  'set.auto': 'Buscar actualizaciones al abrir', 'set.close_game_first': 'Cierra el juego antes de actualizar.',
  'set.privacy': 'Privacidad', 'set.reports': 'Enviar reportes de error automáticos',
  'set.reports_hint': 'Si el juego se cierra con error, se envía un reporte solo para arreglarlo: versión de DrakeStar y de Minecraft, sistema operativo, nombres de mods y las últimas líneas de los registros, con rutas, correos, IPs y claves tachados. Nunca se envían contraseñas ni tokens.',
  'set.language': 'Idioma', 'set.folders': 'Carpetas', 'set.open_data': 'Abrir carpeta de datos', 'set.open_logs': 'Abrir carpeta de registros',
  'set.client_id': 'ID de aplicación de Azure (Microsoft)', 'set.client_id_hint': 'Déjalo vacío para usar el de DrakeStar.', 'set.endpoint': 'Recolector de reportes', 'set.device': 'ID de este equipo', 'set.supported': 'Minecraft compatible', 'set.saved': 'Guardado.',

  'upd.banner': 'Hay una versión nueva de DrakeStar ({v}).', 'upd.install': 'Actualizar y reiniciar', 'upd.busy': 'Termina la partida para actualizar.',

  'log.title': 'Registros', 'log.subtitle': 'Si algo falla, aquí está la causa. Los datos personales aparecen tachados.', 'log.launcher': 'Launcher', 'log.game': 'Juego',
  'log.refresh': 'Actualizar', 'log.empty': 'Todavía no hay registros.', 'log.create': 'Crear reporte', 'log.send': 'Enviar reporte', 'log.sent': 'Reporte enviado. ¡Gracias!', 'log.saved': 'Reporte guardado en {path}',
  'log.live': 'Salida en vivo de esta sesión', 'log.file': 'Último registro guardado del juego',

  'wel.title': 'Bienvenido a DrakeStar', 'wel.lead': 'El launcher de Minecraft de DrakesCraft: elige qué jugar y dale a JUGAR.',
  'wel.s1': 'Inicia sesión con Microsoft o juega sin premium.', 'wel.s2': 'Elige una instancia (por ejemplo DrakesCraft).', 'wel.s3': 'Dale a JUGAR: DrakeStar descarga Java, el juego y los mods por ti.',
  'wel.privacy': 'Enviar reportes de error automáticos (solo para arreglar fallos, con datos personales tachados).', 'wel.start': 'Empezar',

  'err.generic': 'Algo salió mal',
};

const en: Record<string, string> = {
  'import.title': 'Import instance', 'import.path': 'Folder or file path',
  'import.hint': 'Prism: installed folder or exported ZIP. CurseForge: installed instance folder. Modrinth: exported .mrpack. Vanilla and Fabric only.',
  'import.safe': 'Copies mods, settings and worlds to a new instance. Keeps the source and Fabric version. Accounts and startup commands are excluded. Limit: 4 GiB.',
  'import.preview': 'Preview', 'import.copy': 'Import copy', 'import.working': 'Copying and downloading files…',
  'f.memory_hint': 'System RAM: {total}. Recommended: {recommended}, leaving room for the OS and Java.',
  'f.memory_apply': 'Use recommended memory',

  'nav.play': 'Play', 'nav.accounts': 'Accounts', 'nav.instances': 'Instances', 'nav.settings': 'Settings', 'nav.logs': 'Logs',
  'common.cancel': 'Cancel', 'common.save': 'Save', 'common.close': 'Close', 'common.copy': 'Copy', 'common.copied': 'Copied!',
  'common.retry': 'Retry', 'common.open_folder': 'Open folder', 'common.advanced': 'Advanced', 'common.yes': 'Yes', 'common.no': 'No', 'common.sure': 'Sure?',

  'play.button': 'PLAY', 'play.preparing': 'Preparing…', 'play.running': 'Playing', 'play.close_game': 'Close the game',
  'play.account': 'Playing as', 'play.instances': 'Choose what to play', 'play.first_time': 'The first time it downloads Java and the game files; it can take a few minutes.',
  'play.no_account_title': 'Choose how to play', 'play.no_account_text': 'You need an account to start the game.',
  'play.crashed': 'The game closed with an error{code}.', 'play.error': 'The game could not start.', 'play.exited': 'The game closed.', 'play.cancelled': 'Cancelled.',
  'play.report_sent': 'An automatic report was sent so it can be fixed (personal data removed).', 'play.report_saved': 'A report was saved on your PC.',
  'play.view_logs': 'View logs', 'play.disable_fused': 'Turn off Fused', 'play.send_report': 'Send report', 'play.no_instances': 'No instances yet. Create one in the Instances tab.',
  'target.menu': 'Main menu', 'target.server': 'Server {addr}', 'target.local': 'Own server on this PC', 'target.world': 'World “{name}”',

  'acc.title': 'Accounts', 'acc.subtitle': 'You can keep several accounts, premium and offline, and pick which one to play with.',
  'acc.add_ms': 'Sign in with Microsoft', 'acc.add_ms_hint': 'Premium Minecraft Java account. Your browser opens by itself.',
  'acc.add_off': 'Play without premium', 'acc.add_off_hint': 'For single-player/LAN worlds and servers that allow it.',
  'acc.name_ph': 'Player name', 'acc.name_rule': '3 to 16 letters, numbers or _', 'acc.add': 'Add',
  'acc.default': 'Default', 'acc.use': 'Use', 'acc.remove': 'Remove', 'acc.confirm_remove': 'Remove the account {name}? You can add it again later.',
  'acc.microsoft': 'Microsoft', 'acc.offline': 'Offline', 'acc.empty': 'No accounts yet.',
  'acc.temp_id': 'Microsoft sign-in runs in temporary mode until Mojang approves the DrakeStar app. If it fails, use “Play without premium”.',
  'acc.invalid_name': 'Invalid name: use 3 to 16 letters, numbers or _.',

  'msa.title': 'Sign in with Microsoft', 'msa.opening': 'Opening your browser…',
  'msa.waiting': 'Your browser opened. Sign in to your account and, if asked, type this code:',
  'msa.reopen': 'Open the page again', 'msa.working': 'Checking your Minecraft account…', 'msa.expires': 'The code expires in {min}:{sec}', 'msa.failed': 'Sign-in failed',

  'ins.title': 'Instances', 'ins.subtitle': 'Each instance is a Minecraft version with its own mods and saved games folder.',
  'ins.new': 'New instance', 'ins.edit': 'Edit', 'ins.delete': 'Delete', 'ins.confirm_delete': 'Delete the instance “{name}”? Its saved games are not erased from disk.',
  'ins.range': 'Supported: Minecraft {min} to {max}.',
  'f.name': 'Name', 'f.version': 'Minecraft version', 'f.loader': 'Type', 'f.vanilla': 'Vanilla (no mods)', 'f.fabric': 'Fabric (with mods)',
  'f.target': 'When you press PLAY', 'f.t_menu': 'Open the menu', 'f.t_server': 'Join a server', 'f.t_world': 'Open a world',
  'f.address': 'Server address', 'f.world': 'World name', 'f.perf': 'Performance mods', 'f.perf_hint': 'Sodium, Lithium, FerriteCore, ImmediatelyFast and Entity Culling: more FPS.',
  'f.fused': 'DrakeStar Fused (alpha)', 'f.fused_hint': 'Experimental: prepares Slimefun and plugins inside your single-player/LAN world, Minecraft 1.20 to 26.2. It does not run plugins yet.',
  'f.fabric_only': 'Fabric only.', 'f.ram': 'Memory (RAM)', 'f.jvm': 'Java arguments', 'f.note': 'Note', 'f.new_title': 'New instance', 'f.edit_title': 'Edit instance',

  'set.title': 'Settings', 'set.updates': 'Updates', 'set.version': 'Version {v}', 'set.check': 'Check for updates', 'set.checking': 'Checking…',
  'set.uptodate': 'You already have the latest version.', 'set.check_failed': 'Could not check for updates. Check your connection and try again later.', 'set.update_avail': 'A new version is available: {v}', 'set.install': 'Update and restart', 'set.installing': 'Installing…',
  'set.auto': 'Check for updates on start', 'set.close_game_first': 'Close the game before updating.',
  'set.privacy': 'Privacy', 'set.reports': 'Send automatic error reports',
  'set.reports_hint': 'If the game closes with an error, a report is sent only to fix it: DrakeStar and Minecraft versions, operating system, mod names and the last log lines, with paths, emails, IPs and keys removed. Passwords and tokens are never sent.',
  'set.language': 'Language', 'set.folders': 'Folders', 'set.open_data': 'Open data folder', 'set.open_logs': 'Open logs folder',
  'set.client_id': 'Azure application ID (Microsoft)', 'set.client_id_hint': 'Leave empty to use the DrakeStar one.', 'set.endpoint': 'Report collector', 'set.device': 'This device ID', 'set.supported': 'Supported Minecraft', 'set.saved': 'Saved.',

  'upd.banner': 'A new DrakeStar version is available ({v}).', 'upd.install': 'Update and restart', 'upd.busy': 'Finish your game to update.',

  'log.title': 'Logs', 'log.subtitle': 'If something fails, the cause is here. Personal data is removed.', 'log.launcher': 'Launcher', 'log.game': 'Game',
  'log.refresh': 'Refresh', 'log.empty': 'No logs yet.', 'log.create': 'Create report', 'log.send': 'Send report', 'log.sent': 'Report sent. Thank you!', 'log.saved': 'Report saved at {path}',
  'log.live': 'Live output of this session', 'log.file': 'Latest saved game log',

  'wel.title': 'Welcome to DrakeStar', 'wel.lead': 'The DrakesCraft Minecraft launcher: choose what to play and press PLAY.',
  'wel.s1': 'Sign in with Microsoft or play without premium.', 'wel.s2': 'Pick an instance (for example DrakesCraft).', 'wel.s3': 'Press PLAY: DrakeStar downloads Java, the game and the mods for you.',
  'wel.privacy': 'Send automatic error reports (only to fix bugs, personal data removed).', 'wel.start': 'Get started',

  'err.generic': 'Something went wrong',
};

const dict: Record<string, Record<string, string>> = { es, en };

function initial(): string {
  try {
    const saved = localStorage.getItem('drakestar.lang');
    if (saved === 'es' || saved === 'en') return saved;
  } catch { /* sin almacenamiento */ }
  return typeof navigator !== 'undefined' && navigator.language?.toLowerCase().startsWith('en') ? 'en' : 'es';
}

let lang = $state(initial());

export function setLang(l: string) {
  lang = l === 'en' ? 'en' : 'es';
  try { localStorage.setItem('drakestar.lang', lang); } catch { /* sin almacenamiento */ }
  if (typeof document !== 'undefined') document.documentElement.lang = lang;
}
export const currentLang = () => lang;

export function t(key: string, vars?: Record<string, string | number>): string {
  let s = dict[lang][key] ?? es[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}
