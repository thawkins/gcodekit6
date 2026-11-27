# Gerber a G-code

La herramienta Gerber a G-code permite convertir archivos Gerber estándar (comúnmente utilizados en la fabricación de PCB) a G-code para el fresado de aislamiento CNC.

## Uso

1. **Seleccionar directorio**: Haga clic en "Examinar..." para seleccionar la carpeta que contiene sus archivos Gerber. La herramienta detectará y asignará automáticamente los archivos de capas comunes (p. ej., Cobre superior, Perforaciones).

2. **Tipo de capa**: Seleccione la capa que desea procesar en el menú desplegable. El archivo detectado para esa capa se mostrará a continuación.

3. **Parámetros**:

* **Ancho/Alto de la placa**: Dimensiones de su PCB.

* **Desplazamiento X/Y**: Desplaza el origen del G-code.

* **Velocidad de avance**: Velocidad de corte en mm/min.

* **Velocidad del husillo**: RPM del husillo.

* **Profundidad de corte**: Profundidad Z del corte (negativa para cortar en el material).
* **Z seguro**: Altura de retracción para movimientos rápidos.

* **Diámetro de la herramienta**: Diámetro de la broca de grabado (broca en V o fresa).

* **Ancho de aislamiento**: Ancho adicional para dejar espacio alrededor de las pistas.

* **Eliminar exceso de cobre**: (Desgaste) Si está marcada, genera trayectorias para eliminar todo el cobre que no forma parte de las pistas.

4. **Agujeros de alineación**:

* **Generar agujeros de alineación**: Añade operaciones de perforación para los pines de alineación.

* **Diámetro del agujero**: Diámetro de los agujeros de alineación.

* **Margen**: Distancia desde el borde de la placa hasta los agujeros de alineación.

* **Profundidad de taladrado (Z)**: Profundidad de los agujeros de alineación y de la capa Agujeros de taladro (negativa, atravesando la placa).

## Agujeros de taladro

La capa Agujeros de taladro lee el archivo de taladrado Excellon (`.drl`, `.xln`, `.exc`, un nombre con "drill" o un `.txt` que empieza por `M48`) y perfora cada agujero hasta la **Profundidad de taladrado (Z)** a la velocidad de avance. Las brocas se usan de menor a mayor; antes de cada una el husillo se detiene y el programa hace una pausa (`M0`) con un comentario que indica la broca que hay que montar. Reanude cuando esté colocada. Las ranuras se cortan con la broca de extremo a extremo. Los agujeros usan el mismo Desplazamiento X/Y que las capas de cobre, por lo que coinciden con el corte de aislamiento.

## Salida

Haga clic en **Generar G-code** para crear la trayectoria. El resultado se cargará en el editor/visualizador de G-code.
