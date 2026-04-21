# OverLex

> Traduce cualquier texto en pantalla sin salir de tu juego.

## Instalacion rapida

Abre **PowerShell como administrador** y corre:

```powershell
irm https://raw.githubusercontent.com/TasioDemarchi/overlex/main/install.ps1 | iex
```

Eso es todo. El script descarga e instala la ultima version automaticamente.

---

## Que es OverLex

OverLex es una herramienta super ligera que te permite traducir texto de cualquier aplicacion en Windows sin necesidad de minimizar o cambiar de ventana. Imagina que estas jugando un RPG en japones y no entendes que dice un dialogo — con solo presionar una tecla podes capturar ese texto y verlo traducido al instante, todo sin salir del juego.

Esta pensado principalmente para gamers que juegan en ventana sin bordes (borderless windowed) y quieren experimentar juegos en su idioma original sin romperse la cabeza con diccionarios.

### Que hace

- **OCR Capture**: Captura cualquier region de tu pantalla, extrae el texto y lo traduce
- **Write Mode**: Escribi texto directamente para traducirlo al instante
- **Overlay de resultados**: Muestra la traduccion en una ventana semitransparente que no molesta
- **Hotkeys globales**: Activa las funciones desde cualquier aplicacion con teclas personalizadas
- **Configuracion completa**: Idiomas, posicion del overlay, tiempo de cierre automatico, API keys

## Requisitos

- Windows 10 o Windows 11 (64-bit)
- Conexion a internet (para la traduccion)

## Instalacion manual

Si preferis descargar el `.exe` directamente, lo encontras en la pagina de [Releases](https://github.com/TasioDemarchi/overlex/releases/latest).

## Desarrollo local

Si queres compilar el proyecto vos mismo, necesitas:

| Requisito | Link |
|-----------|------|
| **Rust** | [rustup.rs](https://rustup.rs/) |
| **Node.js LTS** | [nodejs.org](https://nodejs.org/) |
| **VS Build Tools** (con "Desktop development with C++") | [visualstudio.microsoft.com](https://visualstudio.microsoft.com/visual-cpp-build-tools/) |

```bash
git clone https://github.com/TasioDemarchi/overlex.git
cd overlex
npm install
npx tauri dev
```

## Como usar OverLex

### Modo OCR (captura de pantalla)

1. Mientras estas en cualquier aplicacion (juego, navegador, documento), presiona **Ctrl+Shift+T**
2. La pantalla se va a "congelar" momentarily con una ligera sombra
3. Haz clic y arrastra para seleccionar la region con el texto que queres traducir
4. Suelta el mouse — el overlay va a desaparecer y vas a volver a tu aplicacion
5. Vas a ver un pequeno overlay con "Traduciendo..." y luego aparecera el resultado

### Modo Write (escribir texto)

1. Presiona **Ctrl+Shift+W**
2. Aparece un campo de texto flotante
3. Escribi lo que queres traducir y presiona Enter
4. El resultado aparecera en un overlay

### Cerrar overlays

- Presiona **Escape** para cerrar cualquier overlay
- Los overlays se cierran automaticamente despues de unos segundos (configurable)

### Menu del sistema (tray)

OverLex corre en segundo plano. Buscala en el area de notificaciones (abajo a la derecha, junto al reloj):

- **Click izquierdo**: Abrir configuracion
- **Click derecho**: Menu con opciones rapidas (Salir, Configuracion,Acerca de)

## Configuracion

Podes configurar todo desde el panel de settings. Haciendo click en el icono del tray y seleccionando "Configuracion" o presionar **Ctrl+Shift+S** cuando la app este corriendo.

### Idiomas

| Setting | Descripcion |
|---------|-------------|
| **Source Language** | Idioma del texto original (ej: Japones, Ingles, Coreano) |
| **Target Language** | Idioma al que queres traducir (ej: Espanol, Ingles) |
| **Auto-detect** | Si esta activado, OverLex intenta adivinar el idioma automaticamente |

### Overlay

| Setting | Descripcion |
|---------|-------------|
| **Position** | Donde aparece el overlay de resultados (esquina superior derecha, inferior derecha, etc.) |
| **Timeout** | Cuantos segundos dura el overlay antes de cerrarse solo (por defecto: 5 segundos) |
| **Opacity** | Que tan transparente es el overlay (0 a 100%) |

### Hotkeys

| Setting | Descripcion |
|---------|-------------|
| **OCR Capture** | Tecla para activar el modo captura (por defecto: Ctrl+Shift+T) |
| **Write Mode** | Tecla para activar el modo escritura (por defecto: Ctrl+Shift+W) |
| **Settings** | Tecla para abrir configuracion (por defecto: Ctrl+Shift+S) |

### API y traduccion

| Setting | Descripcion |
|---------|-------------|
| **Translation Engine** | Que servicio de traduccion usar (LibreTranslate por defecto, gratuito) |
| **API Key** | Clave opcional si usas un servicio premium (DeepL, Google Translate) |
| **Custom Endpoint** | URL personalizada si tenes tu propio servidor de traduccion |

## Estructura del proyecto

Si queres meterte a desarrollar o entender como funciona el codigo, aqui tenes el mapa:

```
overlex/
├── src/                          # Frontend (HTML/CSS/JS)
│   ├── freeze/                   # Pantalla de captura/congelado
│   │   ├── freeze.js            # Logica para seleccionar region
│   │   └── index.html           # Template de la pantalla freeze
│   ├── result/                   # Overlay de resultados
│   │   ├── result.js            # Logica para mostrar traduccion
│   │   └── index.html           # Template del resultado
│   ├── settings/                 # Panel de configuracion
│   │   ├── settings.js          # Logica de settings
│   │   └── index.html           # Template de configuracion
│   └── write/                    # Modo escritura
│       ├── write.js             # Logica del input
│       └── index.html           # Template del modo write
│
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── capture.rs           # Captura de pantalla (Windows API)
│   │   ├── commands.rs          # Comandos Tauri (bridge frontend-backend)
│   │   ├── hotkeys.rs           # Registro de hotkeys globales
│   │   ├── lib.rs               # Exports de la biblioteca
│   │   ├── main.rs              # Punto de entrada de la app
│   │   ├── ocr.rs               # Integracion con Windows OCR
│   │   ├── settings.rs          # Manejo de configuracion
│   │   ├── tray.rs              # Icono del sistema
│   │   └── translation/         # Integracion con APIs de traduccion
│   ├── Cargo.toml               # Dependencias de Rust
│   └── tauri.conf.json          # Configuracion de Tauri
│
├── README.md                     # Este archivo
├── PRD.md                        # Requisitos del producto
└── package.json                  # Dependencias de Node.js
```

## Troubleshooting

### El OCR no detecta el texto

- **Problema**: Selecciono una region pero no aparece nada traduzido
- **Soluciones posibles**:
  - Asegurate de que el texto sea legible y no este en una imagen muy oscura o con poco contraste
  - Verifica que tengas el language pack de Windows instalado para el idioma que estas capturando
  - Intenta seleccionar solo una pequena region a la vez en lugar de toda la pantalla
  - Algunos juegos con efectos especiales (shaders, bloom) pueden dificultar el OCR — proba en otra aplicacion para comparar

### La traduccion falla

- **Problema**: Aparece "Error de traduccion" o no llega el resultado
- **Soluciones posibles**:
  - Verifica que tengas conexion a internet
  - Si usas un API key, asegurate de que este bien escrita en la configuracion
  - LibreTranslate a veces tiene servidores saturados — proba mas tarde o cambia a otro motor en settings
  - Revisa que el idioma origen y destino esten correctamente seleccionados

### Los hotkeys no responden

- **Problema**: Presiono Ctrl+Shift+T pero no pasa nada
- **Soluciones posibles**:
  - Ejecuta OverLex como administrador (click derecho > Ejecutar como administrador)
  - Verifica que ningun otro programa este usando esas mismas teclas
  - Si tenes un teclado con software de macros (Razer, Logitech), desactiva temporalmente las teclas rapidas globales
  - Revisa la configuracion de hotkeys en el panel de settings

### La app no inicia

- **Problema**: Ejecuto npx tauri dev y da error
- **Soluciones posibles**:
  - Ejecuta `cargo check` en src-tauri para ver si hay errores de compilacion
  - Asegurate de tener Rust instalado correctamente: `rustc --version` debe mostrar una version
  - Verifica que VS Build Tools este instalado con la opcion C++
  - Borrar la carpeta node_modules y ejecutar npm install de nuevo

### El overlay tapa mi juego

- **Problema**: El overlay de captura aparece y no puedo ver mi juego
- **Solucion**: Esto es intencional — la pantalla "congelada" te permite seleccionar el texto sin que el juego se mueva. Pero el tiempo de seleccion es muy curto (1-2 segundos), asi que rapidamente volves a tu juego. Si molesta demasiado, reduces el timeout en settings.

### consumes mucha memoria

- **Problema**: OverLex usa mucha RAM y afecta mi juego
- **Solucion**: OverLex esta disenado para usar menos de 50MB en idle. Si estas viendo mucho mas, puede que haya una fuga de memoria. Reinicia la aplicacion desde el tray (click derecho > Salir y luego volve a abrir) y reporta el problema en GitHub.

## Contribuir

Si queres ayudar a mejorar OverLex, sos bienvenido:

1. Hace un fork del repositorio
2. Crea una rama para tu feature: `git checkout -b mi-nueva-feature`
3. Hace tus cambios y commit: `git commit -m 'Agrego nueva feature'`
4. Push a la rama: `git push origin mi-nueva-feature`
5. Abre un Pull Request

## Licencia

MIT — hacer lo que quieras con el codigo, pero sin garantias.