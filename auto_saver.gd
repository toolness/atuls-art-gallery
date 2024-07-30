extends Node

class_name AutoSaver

var player: Player

var player_start_position: Vector3

var player_start_rotation: Vector3

var player_start_teleport_position: Vector3

@onready var SAVE_STATE_FILENAME := ArtObjects.ROOT_DIR + "save_state.json"

const AUTOSAVE_INTERVAL := 30.0

var seconds_since_last_save := 0.0

func delete_state() -> void:
	if not player:
		return
	print("Deleting state.")
	if FileAccess.file_exists(SAVE_STATE_FILENAME):
		DirAccess.remove_absolute(SAVE_STATE_FILENAME)

func save_state() -> void:
	if not player:
		return
	var state := {
		"player_position": vec3_to_array(player.global_position),
		"player_rotation": vec3_to_array(player.global_rotation),
		"player_teleport_position": vec3_to_array(player.teleport_global_transform.origin),
		# TODO: Save/restore teleport rotation too.
	}
	var file := FileAccess.open(SAVE_STATE_FILENAME, FileAccess.WRITE)
	var json_stringified := JSON.stringify(state)
	# print("Writing state: ", json_stringified)
	file.store_string(json_stringified)
	file.close()


func vec3_to_array(vec: Vector3) -> Array:
	return [vec.x, vec.y, vec.z]


func vec3_from_array(array: Variant, default: Vector3) -> Vector3:
	if array is Array:
		var x: float = array[0]
		var y: float = array[1]
		var z: float = array[2]
		return Vector3(x, y, z)
	return default


func load_state() -> void:
	if not player:
		return
	var state: Dictionary = {}
	if FileAccess.file_exists(SAVE_STATE_FILENAME):
		var json_stringified := FileAccess.get_file_as_string(SAVE_STATE_FILENAME)
		# print("Reading state: ", json_stringified)
		state = JSON.parse_string(json_stringified)
	player.global_position = vec3_from_array(state.get("player_position"), player_start_position)
	player.global_rotation = vec3_from_array(state.get("player_rotation"), player_start_rotation)
	player.teleport_global_transform.origin = vec3_from_array(state.get("player_teleport_position"), player_start_teleport_position)


func _on_before_reload(hard_reset: bool):
	if hard_reset:
		delete_state()
	else:
		save_state()


func _ready():
	set_process(false)


func init(new_player: Player) -> void:
	player = new_player
	player_start_position = player.global_position
	player_start_rotation = player.global_rotation
	player_start_teleport_position = player.teleport_global_transform.origin
	set_process(true)
	UserInterface.before_reload.connect(_on_before_reload)
	print("Save state filename is " + SAVE_STATE_FILENAME + ".")
	load_state()


func _process(delta) -> void:
	seconds_since_last_save += delta
	if seconds_since_last_save >= AUTOSAVE_INTERVAL:
		print("Autosaving.")
		save_state()
		seconds_since_last_save = 0.0


func _notification(what):
	if not player:
		return
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		print("Saving state on exit.")
		save_state()
