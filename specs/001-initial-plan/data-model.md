# data-model.md

## Entities

### Device
- id: string (UUID)
- name: string (user-friendly)
- port: string (system path, e.g., /dev/ttyUSB0 or COM3)
- baud: integer
- firmware: string
- capabilities: list of strings
- status: enum (disconnected, connected, error)
 - transport: enum (serial, tcp, udp)
 - host: string (for network transports; hostname or IP)
 - port_number: integer (for network transports)
 - udp_options: object (for UDP-specific behavior, e.g., retries, sequence numbers)

### Job
- id: string (UUID)
- file_path: string
- lines_total: integer
- lines_sent: integer
- progress: float (0.0 - 1.0)
- status: enum (queued, running, paused, completed, failed)
- created_at: datetime

### Settings
- id: string
- default_baud: integer
- recent_ports: list of strings
- ui_prefs: map

## Relationships
- A Device can have many Jobs (history)
- A Job references a Device while running

## Validation rules
- Serial port paths MUST be non-empty
- Baud rate MUST be within device-supported ranges (validate where possible)

