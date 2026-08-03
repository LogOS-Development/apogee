extends Node3D

# Number of altitude shells used for colour-coding density particles.
const ALTITUDE_SHELLS = 10
const EARTH_RADIUS_KM = 6371.0
const VIS_SCALE = 600.0  # Scale factor so particles/arrows are visible from orbit.

@onready var sampler: AtmosphereGridSampler = $AtmosphereGridSampler
@onready var density_mesh: MultiMeshInstance3D = $DensityMultiMesh
@onready var wind_mesh: MultiMeshInstance3D = $WindMultiMesh
@onready var animation_timer: Timer = $AnimationTimer
@onready var model_label: Label = $UI/ModelLabel

var _model_names := ["NRLMSISE-00", "Jacchia-Bowman"]

func _ready() -> void:
	print("[atmosphere] _ready start")
	_build_density_multimesh()
	_build_wind_multimesh()
	sampler.resample()
	_update_density_instances()
	_update_wind_instances()
	print("[atmosphere] _ready done; instances=", density_mesh.multimesh.instance_count)
	animation_timer.timeout.connect(_on_animation_tick)

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		match event.keycode:
			KEY_SPACE:
				var next = (sampler.model_kind + 1) % 2
				sampler.model_kind = next
				model_label.text = "Model: " + _model_names[next]
				sampler.resample()
				_update_density_instances()
				_update_wind_instances()
				print("[atmosphere] switched model to ", _model_names[next])
			KEY_R:
				sampler.resample()
				_update_density_instances()
				_update_wind_instances()
				print("[atmosphere] resampled")
			KEY_A:
				if animation_timer.is_stopped():
					animation_timer.start()
					print("[atmosphere] animation started")
				else:
					animation_timer.stop()
					print("[atmosphere] animation stopped")
			KEY_ESCAPE:
				get_tree().quit()

func _on_animation_tick() -> void:
	sampler.seconds_utc = fmod(sampler.seconds_utc + 3600.0, 86400.0)
	sampler.resample()
	_update_density_instances()
	_update_wind_instances()

func _build_density_multimesh() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.instance_count = sampler.lat_steps * sampler.lon_steps * sampler.alt_steps
	var mesh := SphereMesh.new()
	mesh.radius = 0.5
	mesh.height = 1.0
	mm.mesh = mesh
	density_mesh.multimesh = mm

func _build_wind_multimesh() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = false
	mm.instance_count = sampler.lat_steps * sampler.lon_steps * sampler.alt_steps
	var arrow := CylinderMesh.new()
	arrow.top_radius = 0.15
	arrow.bottom_radius = 0.15
	arrow.height = 1.0
	mm.mesh = arrow
	wind_mesh.multimesh = mm

func _update_density_instances() -> void:
	var data: PackedFloat64Array = sampler.get_samples()
	var max_rho: float = sampler.max_density()
	if max_rho <= 0.0:
		max_rho = 1.0

	var count: int = data.size() / 8
	var alt_range: float = sampler.altitude_max_km - sampler.altitude_min_km

	for i in range(count):
		var base := i * 8
		var alt_km := data[base + 0]
		var lat_rad := data[base + 1]
		var lon_rad := data[base + 2]
		var rho := data[base + 3]

		var pos := _geodetic_to_cartesian(alt_km, lat_rad, lon_rad)
		var size: float = clamp(rho / max_rho, 0.05, 1.0) * VIS_SCALE
		var shell := int(((alt_km - sampler.altitude_min_km) / max(1.0, alt_range)) * ALTITUDE_SHELLS)
		var color := _shell_color(shell)

		density_mesh.multimesh.set_instance_transform(i, Transform3D(Basis.IDENTITY.scaled(Vector3(size, size, size)), pos))
		density_mesh.multimesh.set_instance_color(i, color)

func _update_wind_instances() -> void:
	var data: PackedFloat64Array = sampler.get_samples()
	var max_speed: float = sampler.max_wind_speed()
	if max_speed <= 0.0:
		max_speed = 1.0

	var count: int = data.size() / 8
	for i in range(count):
		var base := i * 8
		var alt_km := data[base + 0]
		var lat_rad := data[base + 1]
		var lon_rad := data[base + 2]
		var e := data[base + 5]
		var n := data[base + 6]
		var u := data[base + 7]

		var pos := _geodetic_to_cartesian(alt_km, lat_rad, lon_rad)
		var length := sqrt(e * e + n * n + u * u)

		if length < 1e-6:
			wind_mesh.multimesh.set_instance_transform(i, Transform3D(Basis.IDENTITY, pos).scaled(Vector3(0.001, 0.001, 0.001)))
			continue

		var world_vec := _enu_to_world(Vector3(e, n, u), lat_rad, lon_rad)
		var arrow_length: float = (length / max_speed) * VIS_SCALE * 2.0

		var up := pos.normalized()
		var forward := world_vec.normalized()
		var right := forward.cross(up)
		if right.length_squared() < 1e-6:
			right = Vector3(1, 0, 0)
		right = right.normalized()
		up = right.cross(forward).normalized()
		var basis := Basis(right, up, forward)
		wind_mesh.multimesh.set_instance_transform(i, Transform3D(basis, pos).scaled(Vector3(1.0, arrow_length, 1.0)))

func _geodetic_to_cartesian(alt_km: float, lat_rad: float, lon_rad: float) -> Vector3:
	var r := EARTH_RADIUS_KM + alt_km
	var x := r * cos(lat_rad) * cos(lon_rad)
	var y := r * cos(lat_rad) * sin(lon_rad)
	var z := r * sin(lat_rad)
	return Vector3(x, z, -y)

func _enu_to_world(enu: Vector3, lat_rad: float, lon_rad: float) -> Vector3:
	var up := Vector3(cos(lat_rad) * cos(lon_rad), sin(lat_rad), cos(lat_rad) * sin(lon_rad))
	var east := up.cross(Vector3(0, 1, 0)).normalized()
	if east.length_squared() < 1e-6:
		east = Vector3(1, 0, 0)
	var north := east.cross(up).normalized()
	return east * enu.x + north * enu.y + up * enu.z

func _shell_color(shell: int) -> Color:
	var hue := float(shell % ALTITUDE_SHELLS) / float(ALTITUDE_SHELLS)
	return Color.from_hsv(hue, 0.8, 0.9)
