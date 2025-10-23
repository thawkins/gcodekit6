# Data Model

Entities

- Device
  - id: string (unique)
  - transport: enum { serial, tcp, udp }
  - port: string (e.g., /dev/ttyUSB0 or 192.168.1.50:8888)
  - firmware: enum { GRBL, Smoothieware, TinyG, G2core, Unknown }
  - capabilities: map<string, bool>

- Job
  - id: string
  - file_path: string
  - lines_total: u64
  - lines_sent: u64
  - progress: float (0.0..1.0)
  - status: enum { Pending, Running, Paused, Completed, Error }
  - created_at: timestamp

- Transport
  - type: enum (serial, tcp, udp)
  - config: object (baud, parity for serial; timeouts for tcp/udp)

- Settings
  - data_dir: path
  - default_baud: u32
  - emergency_stop_command: string

Validation rules
- Job.progress computed as lines_sent / lines_total; lines_total must be > 0.
- Device.port must be a valid OS-specific port string or host:port.
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

