extends Node3D

# Fire spread visualizer for Verdant.
# Renders the fire grid as a top-down MultiMesh with per-cell colors:
#   green  = unburned fuel
#   red    = actively burning
#   dark   = burned out
#   orange = fire front (lfn near zero)

const CELL_VIS_SIZE := 0.08  # world units per cell
const GRID_OFFSET := Vector3(-0.5, 0.0, -0.5)  # center grid at origin

@onready var sampler: Node = $VerdantFireSampler
@onready var fire_mesh: MultiMeshInstance3D = $FireMultiMesh
@onready var camera: Camera3D = $Camera3D
@onready var status_label: Label = $UI/StatusLabel

# Playback
var _playing := false
var _dt := 1.0  # sim seconds per frame
var _step_count := 0

# Camera orbit
var _cam_distance := 3.0
var _cam_yaw := 0.0
var _cam_pitch := deg_to_rad(45.0)
var _drag_button := -1
var _mouse_pos := Vector2.ZERO

func _ready() -> void:
	if sampler == null:
		_log_error("VerdantFireSampler (Rust GDExtension) not loaded.")
		return
	if not sampler.has_method("step"):
		_log_error("VerdantFireSampler missing step() method.")
		return

	_build_fire_multimesh()
	sampler.ignite_point(sampler.grid_width / 2, sampler.grid_height / 2)
	_update_instances()
	_update_status()
	_update_camera_transform()
	print("[verdant] ready: %dx%d grid, fuel=%s" % [
		sampler.grid_width, sampler.grid_height, sampler.fuel_category_name()
	])

func _process(delta: float) -> void:
	if _playing:
		sampler.step(_dt)
		_step_count += 1
		_update_instances()
		_update_status()

func _build_fire_multimesh() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.instance_count = sampler.grid_width * sampler.grid_height
	var mesh := PlaneMesh.new()
	mesh.size = Vector2(CELL_VIS_SIZE, CELL_VIS_SIZE)
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.vertex_color_use_as_albedo = true
	mesh.material = mat
	mm.mesh = mesh
	fire_mesh.multimesh = mm

func _update_instances() -> void:
	var lfn: PackedFloat32Array = sampler.get_lfn()
	var fuel: PackedFloat32Array = sampler.get_fuel_frac()
	var heat: PackedFloat32Array = sampler.get_heat_flux()
	var gw: int = sampler.grid_width
	var gh: int = sampler.grid_height
	var max_heat: float = 0.0
	for i in range(heat.size()):
		var h := heat[i]
		if h > max_heat:
			max_heat = h

	for row in range(gh):
		for col in range(gw):
			var i := row * gw + col
			var x := (float(col) / float(gw) - 0.5) * CELL_VIS_SIZE * gw
			var z := (float(row) / float(gh) - 0.5) * CELL_VIS_SIZE * gh
			var pos := Vector3(x, 0.0, z)

			var l := lfn[i]
			var f := fuel[i]
			var h := heat[i]

			var color: Color
			if l > 0.0:
				# Unburned — green, brightness by fuel
				color = Color(0.1, 0.4 + 0.3 * f, 0.1, 1.0)
			elif f > 0.0:
				# Burning — red/orange by heat flux
				if h > 0.0 and max_heat > 0.0:
					var t: float = clamp(h / max_heat, 0.0, 1.0)
					color = Color(1.0, 0.3 + 0.4 * (1.0 - t), 0.0, 1.0)
				else:
					color = Color(0.8, 0.3, 0.0, 1.0)
			else:
				# Burned out — dark gray
				color = Color(0.15, 0.1, 0.05, 1.0)

			fire_mesh.multimesh.set_instance_transform(i, Transform3D(Basis.IDENTITY, pos))
			fire_mesh.multimesh.set_instance_color(i, color)

func _input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			_cam_distance = maxf(_cam_distance - 0.2, 1.0)
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_cam_distance += 0.2
			_update_camera_transform()
		elif event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				_drag_button = MOUSE_BUTTON_LEFT
				_mouse_pos = event.position
			else:
				_drag_button = -1
	elif event is InputEventMouseMotion:
		_mouse_pos = event.position
		if _drag_button == MOUSE_BUTTON_LEFT:
			_cam_yaw -= event.relative.x * 0.005
			_cam_pitch = clampf(_cam_pitch - event.relative.y * 0.005, deg_to_rad(5.0), deg_to_rad(85.0))
			_update_camera_transform()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		match event.keycode:
			KEY_SPACE:
				_playing = not _playing
				print("[verdant] ", "playing" if _playing else "paused")
			KEY_R:
				sampler.init_fire()
				sampler.ignite_point(sampler.grid_width / 2, sampler.grid_height / 2)
				_step_count = 0
				_update_instances()
				_update_status()
				print("[verdant] reset")
			KEY_ESCAPE:
				get_tree().quit()

func _update_camera_transform() -> void:
	var offset := Vector3(
		_cam_distance * cos(_cam_pitch) * sin(_cam_yaw),
		_cam_distance * sin(_cam_pitch),
		_cam_distance * cos(_cam_pitch) * cos(_cam_yaw)
	)
	camera.position = offset
	camera.look_at(Vector3.ZERO)

func _update_status() -> void:
	status_label.text = "Step %d | Time %.0fs | Burning %d | Burned %.0f m² | %s" % [
		_step_count, sampler.sim_time, sampler.burning_count,
		sampler.burned_area, "PLAYING" if _playing else "PAUSED"
	]

func _log_error(msg: String) -> void:
	push_error(msg)
	print(msg)
	status_label.text = msg
	status_label.modulate = Color.RED