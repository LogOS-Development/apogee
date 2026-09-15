extends Node3D

# Solar system visualizer — move around the system, zoom in on planets.
#
# Controls:
#   Left drag: orbit camera
#   Right drag: pan
#   Scroll: zoom
#   Click a body: focus on it (camera follows)
#   F: toggle follow mode
#   Space: play/pause simulation
#   R: reset to system view
#   1/2/3: switch preset (inner system / earth-moon / earth-only)
#   Esc: quit

@onready var sampler: Node = $SolarSystemView
@onready var camera: Camera3D = $Camera3D
@onready var status_label: Label = $UI/StatusLabel
@onready var body_label: Label = $UI/BodyLabel

var _body_meshes: MultiMeshInstance3D
var _names: PackedStringArray = []
var _radii: PackedFloat32Array = []
var _focused_body := -1
var _follow_mode := false

# Camera state
var _cam_distance := 30.0
var _cam_yaw := 0.0
var _cam_pitch := deg_to_rad(30.0)
var _cam_target := Vector3.ZERO
var _drag_button := -1
var _mouse_pos := Vector2.ZERO
var _playing := true

func _ready() -> void:
	if sampler == null:
		_log_error("SolarSystemView not loaded.")
		return
	if not sampler.has_method("get_positions"):
		_log_error("SolarSystemView missing get_positions().")
		return

	_build_body_multimesh()
	_update_bodies()
	_update_camera_transform()
	print("[solar] ready: %d bodies" % sampler.body_count)

func _process(delta: float) -> void:
	if _playing:
		sampler.step(delta)
	_update_bodies()
	if _follow_mode and _focused_body >= 0:
		var positions: PackedVector3Array = sampler.get_positions()
		if _focused_body < positions.size():
			_cam_target = positions[_focused_body]
			_update_camera_transform()
	_update_status()

func _build_body_multimesh() -> void:
	# We use individual MeshInstance3D nodes for named bodies
	# since MultiMesh doesn't support per-instance mesh size easily.
	# Instead, create one sphere per body as children.
	for child in get_children():
		if child is MeshInstance3D and child.name.begins_with("Body"):
			child.queue_free()

	var names: PackedStringArray = sampler.get_names()
	var radii: PackedFloat32Array = sampler.get_radii()
	_names = names
	_radii = radii

	for i in range(names.size()):
		var mesh := SphereMesh.new()
		var radius: float = max(radii[i], 0.005)
		mesh.radius = radius
		mesh.height = radius * 2.0

		var mat := StandardMaterial3D.new()
		mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		mat.albedo_color = _body_color(i, names.size())
		mesh.material = mat

		var node := MeshInstance3D.new()
		node.name = "Body" + str(i)
		node.mesh = mesh
		add_child(node)

func _update_bodies() -> void:
	var positions: PackedVector3Array = sampler.get_positions()
	for i in range(min(positions.size(), _names.size())):
		var node := get_node_or_null("Body" + str(i))
		if node:
			node.position = positions[i]

func _body_color(i: int, total: int) -> Color:
	# Star = yellow, planets = varied hues
	if i == 0:
		return Color(1.0, 0.9, 0.3)
	var hue := float(i) / float(total)
	return Color.from_hsv(hue, 0.6, 0.9)

func _input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			_cam_distance = maxf(_cam_distance - 0.3, 0.1)
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_cam_distance += 0.3
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				# Check if clicking on a body
				var clicked := _pick_body(event.position)
				if clicked >= 0:
					_focused_body = clicked
					_follow_mode = true
					print("[solar] focused on %s" % _names[clicked])
				else:
					_drag_button = MOUSE_BUTTON_LEFT
					_mouse_pos = event.position
			else:
				_drag_button = -1
		elif event.button_index == MOUSE_BUTTON_RIGHT:
			if event.pressed:
				_drag_button = MOUSE_BUTTON_RIGHT
				_mouse_pos = event.position
			else:
				_drag_button = -1
	elif event is InputEventMouseMotion:
		_mouse_pos = event.position
		if _drag_button == MOUSE_BUTTON_LEFT:
			_cam_yaw -= event.relative.x * 0.005
			_cam_pitch = clampf(_cam_pitch + event.relative.y * 0.005, deg_to_rad(5.0), deg_to_rad(85.0))
			_update_camera_transform()
		elif _drag_button == MOUSE_BUTTON_RIGHT:
			var cam_basis := camera.global_transform.basis
			var scale_factor := 0.003 * _cam_distance
			_cam_target += cam_basis.x * (-event.relative.x * scale_factor)
			_cam_target += cam_basis.y * (event.relative.y * scale_factor)
			_update_camera_transform()

func _pick_body(mouse_pos: Vector2) -> int:
	var ray_origin := camera.project_ray_origin(mouse_pos)
	var ray_dir := camera.project_ray_normal(mouse_pos)
	var positions: PackedVector3Array = sampler.get_positions()
	var best_i := -1
	var best_dist := INF
	for i in range(positions.size()):
		var pos: Vector3 = positions[i]
		var to_point := pos - ray_origin
		var projected := to_point.dot(ray_dir)
		if projected < 0.0:
			continue
		var closest := ray_origin + ray_dir * projected
		var dist := pos.distance_to(closest)
		var radius: float = max(_radii[i], 0.02)
		# Use a generous pick radius
		var pick_radius: float = max(_radii[i], 0.005)
		if dist < pick_radius and projected < best_dist:
			best_dist = projected
			best_i = i
	return best_i

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		match event.keycode:
			KEY_SPACE:
				_playing = not _playing
				print("[solar] ", "playing" if _playing else "paused")
			KEY_F:
				_follow_mode = not _follow_mode
				if not _follow_mode:
					_focused_body = -1
				print("[solar] follow ", "on" if _follow_mode else "off")
			KEY_R:
				_focused_body = -1
				_follow_mode = false
				_cam_target = Vector3.ZERO
				_cam_distance = 2.0
				_update_camera_transform()
				print("[solar] reset view")
			KEY_1:
				sampler.preset = 0
				sampler.init_system()
				_build_body_multimesh()
				_focused_body = -1
				_cam_target = Vector3.ZERO
				print("[solar] preset: earth-moon (DE441)")
			KEY_2:
				sampler.preset = 1
				sampler.init_system()
				_build_body_multimesh()
				_focused_body = -1
				_cam_target = Vector3.ZERO
				print("[solar] preset: inner solar system")
			KEY_3:
				sampler.preset = 2
				sampler.init_system()
				_build_body_multimesh()
				_focused_body = -1
				_cam_target = Vector3.ZERO
				print("[solar] preset: earth-only")
			KEY_ESCAPE:
				get_tree().quit()

func _update_camera_transform() -> void:
	var offset := Vector3(
		_cam_distance * cos(_cam_pitch) * sin(_cam_yaw),
		_cam_distance * sin(_cam_pitch),
		_cam_distance * cos(_cam_pitch) * cos(_cam_yaw)
	)
	camera.position = _cam_target + offset
	camera.look_at(_cam_target)

func _update_status() -> void:
	var focus_text := "None"
	if _focused_body >= 0 and _focused_body < _names.size():
		focus_text = _names[_focused_body]
	status_label.text = "Epoch: %s | Bodies: %d | Focus: %s | %s" % [
		sampler.epoch_str, sampler.body_count, focus_text,
		"PLAYING" if _playing else "PAUSED"
	]

func _log_error(msg: String) -> void:
	push_error(msg)
	print(msg)
	status_label.text = msg
	status_label.modulate = Color.RED