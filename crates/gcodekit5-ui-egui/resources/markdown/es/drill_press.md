# Taladradora de columna

Genera G-code que taladra un agujero. Si la herramienta es tan ancha como el agujero, la
herramienta baja en línea recta. Si la herramienta es más pequeña que el agujero, corta el
agujero con una hélice (interpolación helicoidal).

## Geometría del agujero
- **Diámetro del agujero:** Diámetro del agujero terminado.
- **Centro X:** / **Centro Y:** Posición del centro del agujero.
- **Diámetro de la herramienta:** Diámetro de la broca o fresa. No debe ser mayor que el agujero.

## Profundidad y picoteo
- **Z superior (superficie):** Z de la superficie del material. Todos los cortes empiezan aquí.
- **Z inferior (profundidad):** Z del fondo del agujero. Debe estar por debajo del Z superior.
- **Profundidad de picoteo (0 para ninguna):** Profundidad de cada bajada. Con 0 la herramienta
  taladra de una sola vez. Se rechaza una profundidad tan pequeña que el agujero necesitaría
  más de 10000 picoteos.
  - Taladrado recto: tras cada picoteo la herramienta sube en rápido hasta el Z superior para
    sacar la viruta y vuelve a bajar hasta 0,5 mm por encima del último corte.
  - Corte helicoidal: la profundidad de picoteo es la bajada por vuelta de la hélice (1 mm si
    es 0). Tras la última vuelta la herramienta corta un círculo completo más en el fondo y
    vuelve al centro.

## Ajustes de la máquina
- **Velocidad de penetración (mm/min):** Avance de las bajadas rectas.
- **Avance (mm/min):** Avance de las vueltas helicoidales.
- **Velocidad del husillo (RPM):** Velocidad del husillo; se arranca con `M3` y se para con `M5`
  al final.
- **Z seguro:** Altura de los desplazamientos y de la retirada final. Debe estar por encima del
  Z superior.
- **Hacer 'Homing' antes de iniciar:** Añade `$H` al inicio del trabajo. Los desplazamientos y
  retiradas usan entonces coordenadas de máquina (`G53`) para no subir nunca por encima de la
  posición de 'Homing'.

## Botones
- **Cargar** / **Guardar:** Carga o guarda los ajustes de la página como preajuste.
- **Generar:** Crea el G-code.
- **Enviar al editor:** Sustituye el programa del Editor de G-code por el G-code generado.
- **Vista previa:** Sustituye el programa del editor y lo muestra en el Visualizador.

## Notas
- La herramienta no funciona mientras el dispositivo está en modo láser (`$32=1`). Cambie el
  dispositivo a modo CNC primero.
- La herramienta necesita un dispositivo con al menos 3 ejes (X, Y, Z).

## Relacionado
- [Herramientas CAM](help:cam_tools)
- [Índice](help:index)
