# Creador de cajas con pestañas
Genera un diseño de caja con uniones dentadas y produce G-code para corte láser o fresado CNC.
El generador crea múltiples paneles 2D (laterales, tapa/base, separadores) y los organiza en un diseño plano para el corte.

## Entradas clave
### Dimensiones de la caja
- **X (Ancho), Y (Profundidad), H (Alto)**: Tamaño total de la caja.
- **Dimensiones exteriores**: Si está habilitada, las dimensiones se consideran exteriores; de lo contrario, son interiores.

### Configuración de la caja
- **Tipo de caja**: Permite eliminar caras específicas (sin tapa, sin base, etc.).
- **Separadores X/Y**: Número de separadores internos.
- **Clave de separadores**: Indica qué caras incluyen ranuras/pestañas para los separadores.
- **Optimizar diseño**: Agrupa los paneles en un diseño compacto.

### Ajustes del material
- **Espesor**: Espesor del material.
- **Diámetro de corte/herramienta**: Compensación de corte/herramienta.

### Ajustes de las uniones de dedos
- **Ancho del dedo/Ancho del espacio**: Expresado como múltiplos del espesor.
- **Espacios circundantes**: Espacio adicional cerca de los bordes.
- **Juego**: Tolerancia de ajuste.
- **Longitud adicional**: Añade longitud a las pestañas para un ajuste más preciso.
- **Estilo de Dedos**: Rectos, Muelles, Púas o Encaje a Presión.

### Ajustes del láser
- **Pasadas/Potencia (S)/Velocidad de avance**: Parámetros de corte (tipo láser), utilizados al generar el G-code final.
- **Paso en Z**: Cuánto baja Z en cada pasada después de la primera.

### Desplazamientos del origen de trabajo
Desplaza todo el diseño y, opcionalmente, realiza un posicionamiento inicial antes de comenzar (**Hacer 'Homing' antes de iniciar**).

## Optimizar el diseño
El optimizador agrupa los paneles, con una separación de 5 mm entre piezas. Previsualice el programa para comprobar que cabe en su máquina.

## Flujo de trabajo
1. Definir las dimensiones de la caja y el grosor del material.
2. Configurar las uniones y las opciones de divisores.
3. Pulsar **Generar** para crear el programa y después **Enviar al editor**, o **Vista previa** para verlo en el Visualizador. **Generar Formas** añade los contornos de los paneles al Diseñador.
4. Cortar los paneles y ensamblarlos.

## Relacionado
- [Herramientas CAM](help:cam_tools)
- [Visualizador](help:visualizer)
- [Índice](help:index)
