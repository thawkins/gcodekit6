# Configuración del dispositivo
En la configuración del dispositivo, **recupera, inspecciona y gestiona los ajustes del firmware** (p. ej., GRBL `$$`) y las **capacidades** derivadas de dichos ajustes. El comando **Ajustes del firmware** abre esta página.

## Qué verá
- **Ajustes**: una lista de ajustes `$$` que se puede buscar y filtrar, agrupados por categoría.
- **Descripción**: el significado del ajuste seleccionado.

Los detalles del dispositivo (nombre/firmware/versión) están en una ventana aparte: Máquina > **Información del dispositivo**.

## Recuperar ajustes (primer paso recomendado)
1. Conéctese a la máquina en **Control de máquina**.
2. Haga clic en **Obtener** para leer `$$` del controlador.
3. Utilice el cuadro **Filtro:** y el filtro **Categoría:** para localizar ajustes específicos.
Los ajustes recuperados se almacenan para que otras partes de la aplicación puedan utilizarlos (p. ej., recorrido máximo, velocidad de avance máxima, parámetros del husillo/láser).

## Trabajar con ajustes
### Buscar y filtrar
- Usa **Filtro:** para encontrar el número, nombre o descripción del ajuste.
- Usa **Categoría:** para acceder rápidamente a áreas como límites de movimiento, aceleración, husillo, etc.

### Editar
- Algunos ajustes pueden ser de **solo lectura** según el firmware/controlador.
- Haga clic en la fila de un ajuste modificable para abrir el diálogo **Editar ajuste**, introduzca el valor y pulse **Aplicar** para escribirlo en el controlador.

## Guardar y cargar
- **Guardar** exporta los ajustes actuales a un archivo para realizar copias de seguridad o compartirlos.
- **Cargar** importa un archivo de ajustes para que puedas compararlo con una configuración válida.

## Funcionalidades (derivadas)
Algunos comportamientos de la interfaz de usuario se derivan de los ajustes, por ejemplo:
- Posicionamiento inicial habilitado/deshabilitado (p. ej., GRBL `$22`)
- Modo láser (p. ej., GRBL `$32`)

## Notas de seguridad
Cambiar los ajustes del firmware puede afectar inmediatamente al movimiento, los límites y las funciones de seguridad.
- Si no está seguro de alguna configuración, **guarde una copia de seguridad** primero.
- Después de realizar cambios, valide los límites y el comportamiento de posicionamiento a baja velocidad.

## Relacionado
- [Administrador de dispositivos](help:device_manager)
- [Consola de dispositivos](help:device_console)
- [Control de máquina](help:machine_control)
- [Índice](help:index)
