extends Node

class_name AutoSaver

var player: Player

var player_start_position: Vector3

var player_start_rotation: Vector3

var player_start_teleport_position: Vector3

const AUTOSAVE_INTERVAL := 30.0

var seconds_since_last_save := 0.0

func save_state() -> void:
	if not player:
		return
	PersistedConfig.set_vec3(
		PersistedConfig.PLAYER_POSITION,
		player.global_position,
	)
	PersistedConfig.set_vec3(
		PersistedConfig.PLAYER_ROTATION,
		player.global_rotation,
	)
	PersistedConfig.set_vec3(
		PersistedConfig.PLAYER_TELEPORT_POSITION,
		player.teleport_global_transform.origin,
	)
	PersistedConfig.save()


func load_state() -> void:
	if not player:
		return
	player.global_position = PersistedConfig.get_vec3(
		PersistedConfig.PLAYER_POSITION,
		player_start_position,
	)
	player.global_rotation = PersistedConfig.get_vec3(
		PersistedConfig.PLAYER_ROTATION,
		player_start_rotation,
	)
	player.teleport_global_transform.origin = PersistedConfig.get_vec3(
		PersistedConfig.PLAYER_TELEPORT_POSITION,
		player_start_teleport_position,
	)


func _on_before_reload(hard_reset: bool):
	if hard_reset:
		PersistedConfig.delete_player_settings()
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
	load_state()


func _process(delta) -> void:
	seconds_since_last_save += delta
	if seconds_since_last_save >= AUTOSAVE_INTERVAL:
		print("Autosaving player settings.")
		save_state()
		seconds_since_last_save = 0.0


func _notification(what):
	if not player:
		return
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		print("Saving player settings on exit.")
		save_state()
