extends Node3D

const ALTITUDE_SHELLS = 10
const EARTH_RADIUS_KM = 6371.0
const NORMALIZE = 1.0 / EARTH_RADIUS_KM
const VIS_SCALE = 0.08
const SECONDS_PER_DAY := 86400.0

@onready var sampler: Node3D = $AtmosphereGridSampler
@onready var density_mesh: MultiMeshInstance3D = $DensityMultiMesh
@onready var wind_mesh: MultiMeshInstance3D = $WindMultiMesh
@onready var earth_mesh: MeshInstance3D = $Earth
@onready var camera: Camera3D = $Camera3D
@onready var model_label: Label = $UI/ModelLabel
@onready var status_label: Label = $UI/StatusLabel

var _model_names := ["NRLMSISE-00", "Jacchia-Bowman"]
var _layer_names := ["Density", "Wind", "Earth"]
var _layer_visible := [true, true, true]

# Playback state
var _playing := false
var _speed_sim_s_per_real_s := 3600.0  # one sim hour per real second by default
var _sim_seconds_utc := 43200.0        # 12:00 UTC
var _sim_doy := 80
var _start_seconds_utc := 43200.0
var _start_doy := 80

# Camera state (spherical orbit + pan offset)
var _cam_distance := 4.0
var _cam_yaw := deg_to_rad(45.0)
var _cam_pitch := deg_to_rad(30.0)
var _cam_pan := Vector3.ZERO
var _mouse_pos := Vector2.ZERO
var _drag_button := -1

# UI references (filled in _build_ui)
var _play_button: Button
var _time_slider: HSlider
var _time_label: Label
var _speed_label: Label
var _speed_slider: HSlider
var _doy_box: SpinBox
var _mouse_overlay: Control
var _mouse_text: Label

func _ready() -> void:
	print("[atmosphere] _ready start")

	if sampler == null:
		_log_error("AtmosphereGridSampler (Rust GDExtension) not loaded. Rebuild libapogee_godot.so and ensure addons/apogee_godot/apogee_godot.gdextension is present.")
		return
	if not sampler.has_method("resample"):
		_log_error("AtmosphereGridSampler missing resample() method.")
		return
	if not sampler.has_method("get_samples"):
		_log_error("AtmosphereGridSampler missing get_samples() method.")
		return

	_build_density_multimesh()
	_build_wind_multimesh()
	_apply_time_to_sampler()
	_resample()
	_update_instances()
	_update_layer_visibility()
	_update_status()
	_build_ui()
	_update_camera_transform()

	print("[atmosphere] _ready done; instances=", density_mesh.multimesh.instance_count)

func _process(delta: float) -> void:
	if _playing:
		_sim_seconds_utc += _speed_sim_s_per_real_s * delta
		while _sim_seconds_utc >= SECONDS_PER_DAY:
			_sim_seconds_utc -= SECONDS_PER_DAY
			_sim_doy = wrapi(_sim_doy + 1, 1, 367)
		_apply_time_to_sampler()
		_resample()
		_update_instances()
		_update_status()
		_update_time_ui()

	_update_mouse_overlay()

func _apply_time_to_sampler() -> void:
	sampler.seconds_utc = _sim_seconds_utc
	sampler.day_of_year = _sim_doy

func _resample() -> void:
	sampler.resample()

func _get_samples() -> PackedFloat64Array:
	return sampler.get_samples()

func _max_density() -> float:
	return sampler.max_density()

func _max_wind_speed() -> float:
	return sampler.max_wind_speed()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			_cam_distance = maxf(_cam_distance - 0.2 * (1.5 if event.shift_pressed else 1.0), 1.2)
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_cam_distance += 0.2 * (1.5 if event.shift_pressed else 1.0)
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_LEFT or event.button_index == MOUSE_BUTTON_RIGHT:
			if event.pressed:
				_drag_button = event.button_index
				_mouse_pos = event.position
			else:
				_drag_button = -1
	elif event is InputEventMouseMotion:
		_mouse_pos = event.position
		if _drag_button == MOUSE_BUTTON_LEFT:
			# Orbit
			var dx: float = event.relative.x * 0.005
			var dy: float = event.relative.y * 0.005
			_cam_yaw -= dx
			_cam_pitch = clampf(_cam_pitch - dy, deg_to_rad(5.0), deg_to_rad(85.0))
			_update_camera_transform()
		elif _drag_button == MOUSE_BUTTON_RIGHT:
			# Pan in camera plane
			var cam_basis := camera.global_transform.basis
			var scale := 0.003 * _cam_distance
			_cam_pan += cam_basis.x * (-event.relative.x * scale)
			_cam_pan += cam_basis.y * (event.relative.y * scale)
			_update_camera_transform()
	elif event is InputEventKey and event.pressed:
		match event.keycode:
			KEY_SPACE:
				_switch_model()
			KEY_R:
				_resample_and_update()
				print("[atmosphere] resampled")
			KEY_1:
				_toggle_layer(0)
			KEY_2:
				_toggle_layer(1)
			KEY_3:
				_toggle_layer(2)
			KEY_HOME:
				_reset_camera()
			KEY_ESCAPE:
				get_tree().quit()

func _reset_camera() -> void:
	_cam_distance = 4.0
	_cam_yaw = deg_to_rad(45.0)
	_cam_pitch = deg_to_rad(30.0)
	_cam_pan = Vector3.ZERO
	_update_camera_transform()

func _update_camera_transform() -> void:
	var offset := Vector3(
		_cam_distance * cos(_cam_pitch) * sin(_cam_yaw),
		_cam_distance * sin(_cam_pitch),
		_cam_distance * cos(_cam_pitch) * cos(_cam_yaw)
	)
	camera.position = _cam_pan + offset
	camera.look_at_from_position(camera.position, _cam_pan)

func _switch_model() -> void:
	var next = (int(sampler.model_kind) + 1) % 2
	sampler.model_kind = next
	model_label.text = "Model: " + _model_names[next]
	_resample_and_update()
	print("[atmosphere] switched model to ", _model_names[next])

func _resample_and_update() -> void:
	_apply_time_to_sampler()
	_resample()
	_update_instances()
	_update_status()

func _toggle_layer(idx: int) -> void:
	_layer_visible[idx] = not _layer_visible[idx]
	_update_layer_visibility()
	_update_status()
	print("[atmosphere] toggled ", _layer_names[idx], " ", "on" if _layer_visible[idx] else "off")

func _update_layer_visibility() -> void:
	density_mesh.visible = _layer_visible[0]
	wind_mesh.visible = _layer_visible[1]
	earth_mesh.visible = _layer_visible[2]

func _update_instances() -> void:
	_update_density_instances()
	_update_wind_instances()

func _update_status() -> void:
	var hh := int(_sim_seconds_utc) / 3600
	var mm := (int(_sim_seconds_utc) % 3600) / 60
	var ss := int(_sim_seconds_utc) % 60
	var layers := ""
	for i in range(3):
		if _layer_visible[i]:
			layers += _layer_names[i][0]
		else:
			layers += "-"
	status_label.text = "DOY %03d %02d:%02d:%02d UTC | Layers: %s | Speed: %.0f s/s | %s" % [
		_sim_doy, hh, mm, ss, layers, _speed_sim_s_per_real_s,
		"PLAYING" if _playing else "PAUSED"
	]

func _build_density_multimesh() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.instance_count = sampler.lat_steps * sampler.lon_steps * sampler.alt_steps
	var mesh := SphereMesh.new()
	mesh.radius = 1.0
	mesh.height = 2.0
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.vertex_color_use_as_albedo = true
	mesh.material = mat
	mm.mesh = mesh
	density_mesh.multimesh = mm

func _build_wind_multimesh() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = false
	mm.instance_count = sampler.lat_steps * sampler.lon_steps * sampler.alt_steps
	var arrow := CylinderMesh.new()
	arrow.top_radius = 0.3
	arrow.bottom_radius = 0.3
	arrow.height = 1.0
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.albedo_color = Color(0.9, 0.9, 0.2)
	arrow.material = mat
	mm.mesh = arrow
	wind_mesh.multimesh = mm

func _update_density_instances() -> void:
	var data: PackedFloat64Array = _get_samples()
	var max_rho: float = _max_density()
	if max_rho <= 0.0:
		max_rho = 1.0

	var count: int = data.size() / 8
	var alt_min: float = sampler.altitude_min_km
	var alt_max: float = sampler.altitude_max_km
	var alt_range: float = alt_max - alt_min
	if alt_range <= 0.0:
		alt_range = 1.0

	for i in range(count):
		var base := i * 8
		var alt_km := data[base + 0]
		var lat_rad := data[base + 1]
		var lon_rad := data[base + 2]
		var rho := data[base + 3]

		var pos := _geodetic_to_cartesian(alt_km, lat_rad, lon_rad)
		var size: float = clamp(rho / max_rho, 0.05, 1.0) * VIS_SCALE
		var shell := int(((alt_km - alt_min) / alt_range) * float(ALTITUDE_SHELLS))
		shell = clampi(shell, 0, ALTITUDE_SHELLS - 1)
		var color := _shell_color(shell)

		density_mesh.multimesh.set_instance_transform(i, Transform3D(Basis.IDENTITY.scaled(Vector3(size, size, size)), pos))
		density_mesh.multimesh.set_instance_color(i, color)

func _update_wind_instances() -> void:
	var data: PackedFloat64Array = _get_samples()
	var max_speed: float = _max_wind_speed()
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
		var arrow_length: float = clamp(length / max_speed, 0.2, 1.0) * VIS_SCALE * 4.0

		var up := pos.normalized()
		var forward := world_vec.normalized()
		var right := forward.cross(up)
		if right.length_squared() < 1e-6:
			right = Vector3(1, 0, 0) if abs(up.x) < 0.9 else Vector3(0, 1, 0)
			right = (right - up * right.dot(up)).normalized()
		right = right.normalized()
		up = right.cross(forward).normalized()
		if up.length_squared() < 1e-6:
			up = forward.cross(right).normalized()
		var basis := Basis(right, up, forward)
		wind_mesh.multimesh.set_instance_transform(i, Transform3D(basis, pos).scaled(Vector3(1.0, arrow_length, 1.0)))

func _geodetic_to_cartesian(alt_km: float, lat_rad: float, lon_rad: float) -> Vector3:
	var r := (EARTH_RADIUS_KM + alt_km) * NORMALIZE
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

func _log_error(msg: String) -> void:
	push_error(msg)
	print(msg)
	status_label.text = msg
	status_label.modulate = Color.RED

# -----------------------------------------------------------------------------
# Mouse overlay: inspect nearest sample under cursor
# -----------------------------------------------------------------------------
func _update_mouse_overlay() -> void:
	var ray_origin := camera.project_ray_origin(_mouse_pos)
	var ray_dir := camera.project_ray_normal(_mouse_pos)
	var data: PackedFloat64Array = _get_samples()
	var count: int = data.size() / 8
	if count == 0:
		_mouse_overlay.visible = false
		return

	var best_i := -1
	var best_dist := INF
	for i in range(count):
		var base := i * 8
		var alt_km := data[base + 0]
		var lat_rad := data[base + 1]
		var lon_rad := data[base + 2]
		var pos := _geodetic_to_cartesian(alt_km, lat_rad, lon_rad)
		var to_point := pos - ray_origin
		var projected_len := to_point.dot(ray_dir)
		if projected_len < 0.0:
			continue  # behind camera
		var closest := ray_origin + ray_dir * projected_len
		var dist := pos.distance_to(closest)
		if dist < best_dist:
			best_dist = dist
			best_i = i

	var threshold := 0.15 * _cam_distance  # generous because points are small
	if best_i < 0 or best_dist > threshold:
		_mouse_overlay.visible = false
		return

	var base := best_i * 8
	var alt_km := data[base + 0]
	var lat_deg := rad_to_deg(data[base + 1])
	var lon_deg := rad_to_deg(data[base + 2])
	var rho := data[base + 3]
	var temperature := data[base + 4]
	var e := data[base + 5]
	var n := data[base + 6]
	var u := data[base + 7]
	var wind_speed := sqrt(e * e + n * n + u * u)

	_mouse_text.text = (
		"Alt: %.1f km\nLat: %.2f°\nLon: %.2f°\nDensity: %.3e kg/m³\nTemp: %.1f K\nWind E/N/U: %.1f / %.1f / %.1f m/s\nWind speed: %.1f m/s" % [
			alt_km, lat_deg, lon_deg, rho, temperature, e, n, u, wind_speed
		]
	)
	_mouse_overlay.visible = true
	_mouse_overlay.position = _mouse_pos + Vector2(16, 16)
	# Keep inside viewport
	var vp_size: Vector2i = get_viewport().size
	if _mouse_overlay.position.x + _mouse_overlay.size.x > vp_size.x:
		_mouse_overlay.position.x = _mouse_pos.x - _mouse_overlay.size.x - 16
	if _mouse_overlay.position.y + _mouse_overlay.size.y > vp_size.y:
		_mouse_overlay.position.y = _mouse_pos.y - _mouse_overlay.size.y - 16

# -----------------------------------------------------------------------------
# Playback UI
# -----------------------------------------------------------------------------
func _build_ui() -> void:
	var ui := $UI
	var x := 16.0
	var y := 200.0
	var btn_w := 80.0
	var btn_h := 30.0
	var gap := 10.0

	_play_button = Button.new()
	_play_button.text = "Play"
	_play_button.position = Vector2(x, y)
	_play_button.size = Vector2(btn_w, btn_h)
	_play_button.pressed.connect(_on_play_pause)
	ui.add_child(_play_button)

	var rewind_btn := Button.new()
	rewind_btn.text = "Rewind"
	rewind_btn.position = Vector2(x + btn_w + gap, y)
	rewind_btn.size = Vector2(btn_w, btn_h)
	rewind_btn.pressed.connect(_on_rewind)
	ui.add_child(rewind_btn)

	var doy_lbl := Label.new()
	doy_lbl.text = "DOY"
	doy_lbl.position = Vector2(x + 2 * (btn_w + gap), y)
	doy_lbl.size = Vector2(40, btn_h)
	ui.add_child(doy_lbl)

	_doy_box = SpinBox.new()
	_doy_box.min_value = 1
	_doy_box.max_value = 366
	_doy_box.value = _sim_doy
	_doy_box.position = Vector2(x + 2 * (btn_w + gap) + 40, y)
	_doy_box.size = Vector2(90, btn_h)
	_doy_box.value_changed.connect(_on_doy_changed)
	ui.add_child(_doy_box)

	var row2 := y + btn_h + gap

	_speed_label = Label.new()
	_speed_label.text = _format_speed_label()
	_speed_label.position = Vector2(x, row2)
	_speed_label.size = Vector2(300, 24)
	ui.add_child(_speed_label)

	_speed_slider = HSlider.new()
	_speed_slider.min_value = 0.0
	_speed_slider.max_value = SECONDS_PER_DAY
	_speed_slider.value = _speed_sim_s_per_real_s
	_speed_slider.step = 60.0
	_speed_slider.position = Vector2(x, row2 + 24)
	_speed_slider.size = Vector2(300, 16)
	_speed_slider.value_changed.connect(_on_speed_changed)
	ui.add_child(_speed_slider)

	var row3 := row2 + 50

	_time_label = Label.new()
	_time_label.text = _format_time_label()
	_time_label.position = Vector2(x, row3)
	_time_label.size = Vector2(300, 24)
	ui.add_child(_time_label)

	_time_slider = HSlider.new()
	_time_slider.min_value = 0.0
	_time_slider.max_value = SECONDS_PER_DAY
	_time_slider.value = _sim_seconds_utc
	_time_slider.step = 60.0
	_time_slider.position = Vector2(x, row3 + 24)
	_time_slider.size = Vector2(300, 16)
	_time_slider.value_changed.connect(_on_time_changed)
	ui.add_child(_time_slider)

	# Mouse overlay panel
	_mouse_overlay = PanelContainer.new()
	_mouse_overlay.visible = false
	_mouse_overlay.z_index = 100
	var style := StyleBoxFlat.new()
	style.bg_color = Color(0.0, 0.0, 0.0, 0.75)
	style.corner_radius_top_left = 4
	style.corner_radius_top_right = 4
	style.corner_radius_bottom_left = 4
	style.corner_radius_bottom_right = 4
	_mouse_overlay.add_theme_stylebox_override("panel", style)
	ui.add_child(_mouse_overlay)

	_mouse_text = Label.new()
	_mouse_text.add_theme_font_size_override("font_size", 14)
	_mouse_text.add_theme_color_override("font_color", Color.WHITE)
	_mouse_overlay.add_child(_mouse_text)

func _format_time_label() -> String:
	var hh := int(_sim_seconds_utc) / 3600
	var mm := (int(_sim_seconds_utc) % 3600) / 60
	var ss := int(_sim_seconds_utc) % 60
	return "UTC: %02d:%02d:%02d" % [hh, mm, ss]

func _format_speed_label() -> String:
	return "Speed: %.0f sim s / real s" % _speed_sim_s_per_real_s

func _update_time_ui() -> void:
	_time_slider.value = _sim_seconds_utc
	_time_label.text = _format_time_label()
	_doy_box.value = _sim_doy

func _on_play_pause() -> void:
	_playing = not _playing
	_play_button.text = "Pause" if _playing else "Play"
	_update_status()

func _on_rewind() -> void:
	_playing = false
	_play_button.text = "Play"
	_sim_seconds_utc = _start_seconds_utc
	_sim_doy = _start_doy
	_apply_time_to_sampler()
	_resample()
	_update_instances()
	_update_status()
	_update_time_ui()

func _on_speed_changed(value: float) -> void:
	_speed_sim_s_per_real_s = value
	_speed_label.text = _format_speed_label()
	_update_status()

func _on_time_changed(value: float) -> void:
	_playing = false
	_play_button.text = "Play"
	_sim_seconds_utc = value
	_apply_time_to_sampler()
	_resample()
	_update_instances()
	_update_status()
	_update_time_ui()

func _on_doy_changed(value: float) -> void:
	_playing = false
	_play_button.text = "Play"
	_sim_doy = int(value)
	_apply_time_to_sampler()
	_resample()
	_update_instances()
	_update_status()
	_update_time_ui()
