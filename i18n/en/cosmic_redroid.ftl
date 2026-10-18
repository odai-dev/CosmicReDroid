# Application Header & Branding
app-title = CosmicReDroid
active-containers = Active Containers: { $count }
refresh-health = Refresh Health
new-instance = New Instance

# Navigation
nav-instances = Instances
nav-image-factory = Image Factory
nav-snapshots = Snapshots
nav-host-health = Host Health

# Instance States
status-stopped = Stopped
status-starting = Starting
status-waiting-boot = Waiting for Boot
status-running = Running
status-stopping = Stopping
status-cloning = Cloning
status-snapshotting = Snapshotting
status-error = Error: { $message }

# Instance Actions
btn-start = Start
btn-stop = Stop
btn-open-screen = Open Screen
btn-clone = Clone
btn-snapshot = Snapshot
btn-configure = Configure
btn-delete = Delete
btn-save = Save
btn-cancel = Cancel

# Dashboard Card Details
label-android-version = Android { $version }
label-port = ADB Port: { $port }
label-binder = Binder Index: { $index }
label-cpu = CPU
label-ram = RAM

# Configuration Drawer
drawer-title = Configure Instance
drawer-hardware-preset = Hardware Preset
drawer-manufacturer = Manufacturer
drawer-brand = Brand
drawer-model = Model
drawer-device = Device
drawer-board = Board
drawer-fingerprint = Build Fingerprint
drawer-serial = Serial Number
drawer-imei = IMEI
drawer-android-id = Android ID
drawer-mac-address = MAC Address
drawer-display-resolution = Screen Resolution
drawer-fps = Target FPS
drawer-randomize = Randomize Hardware Identity
drawer-proxy-title = HTTP Proxy Configuration
drawer-proxy-enable = Enable Proxy
drawer-proxy-host = Host
drawer-proxy-port = Port

# Image Factory
image-factory-title = ReDroid Image Factory
image-factory-version = Target Android Version
image-factory-opt-libndk = libndk ARM64 Translation
image-factory-opt-gapps = MindTheGapps Package
image-factory-opt-magisk = Magisk Root Framework
image-factory-dockerfile-preview = Generated Dockerfile Preview
image-factory-build-btn = Build Image
image-factory-building = Building...
image-factory-success = Image built successfully.
image-factory-failed = Image build failed.

# Snapshots View
snapshots-title = Storage & Snapshots
snapshots-empty = No snapshots available for this instance.
snapshots-restore-btn = Restore
snapshots-delete-btn = Delete Snapshot
snapshots-size = Size: { $size }

# Host Diagnostics & Health
host-health-title = Host System Diagnostics
check-docker = Docker Daemon Connection
check-binder = Android Binderfs Kernel Nodes (/dev/binder)
check-dri = DRI GPU Render Node (/dev/dri/renderD128)
check-scrcpy = scrcpy Display Utility
check-adb = Android Debug Bridge (adb)
check-reflink = CoW Reflink Filesystem Support
btn-remediate = Remediate Host Issues (pkexec)
remediation-running = Executing remediation script...
remediation-success = Host remediation completed successfully.
remediation-failed = Host remediation failed.
