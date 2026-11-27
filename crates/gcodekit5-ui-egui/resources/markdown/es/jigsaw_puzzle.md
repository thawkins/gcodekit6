# Generador de puzle

Crea un patrón de corte tipo puzle como formas en el Diseñador.

Esta herramienta está diseñada principalmente para **cortadoras láser** (o herramientas CNC muy pequeñas) y produce contornos 2D de las piezas del puzle que encajan entre sí.

## Qué genera
- Un rectángulo exterior (el límite general del puzle)
- Líneas de corte internas que forman las piezas, utilizando un patrón de conectores de puzle pseudoaleatorio

## Parámetros clave
### Dimensiones del puzzle
- **Anchura / Altura**: Tamaño total del puzle terminado (50 a 2000 mm).
- **Radio de esquina**: Redondea las esquinas exteriores (0 a 10 mm).

### Configuración de Matriz
- **Piezas en ancho / Piezas en alto**: Cantidad de piezas en cada dirección (2 a 20). Más piezas implican más cortes internos. Cada pieza debe medir al menos 15 mm.

### Parámetros del Puzzle
- **Tamaño de la pestaña (%)**: Controla el tamaño del conector (10 a 30).
- **Fluctuación (%)**: Controla la variación del conector (0 a 13).
- **Semilla aleatoria**: Cambia la forma de las piezas manteniendo la misma cuadrícula. Usa **Aleatorio** para probar rápidamente nuevos patrones.

## Flujo de trabajo
1. Elige el tamaño del puzle y la cantidad de piezas.
2. Configura el tamaño del conector.
3. Pulsa **Generar**. Las piezas se añaden al Diseñador como formas agrupadas y el Diseñador pasa al frente.
4. En el Diseñador, configura los parámetros del láser y usa **Generar G-Code** para crear el programa.

## Notas
- Para el corte CNC: tenga en cuenta el diámetro de la herramienta y considere reducir la cantidad de piezas.
- Siempre previsualice el resultado antes de cortar.

## Relacionado
- [Herramientas CAM](help:cam_tools)
- [Visualizador](help:visualizer)
- [Índice](help:index)
