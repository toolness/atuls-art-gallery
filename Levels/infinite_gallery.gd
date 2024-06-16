class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@onready var player: Player = $Player

@onready var player_start_position: Vector3 = player.global_position

@onready var gallery_chunks: Array[Moma] = []

const GALLERY_CHUNK_WIDTH = 28


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


func sync_galleries() -> void:
	var middle_gallery_id := get_gallery_id(player.position.x)
	var min_gallery_id := middle_gallery_id - 1
	var max_gallery_id := middle_gallery_id + 1

	# Get rid of galleries that are far from the player.
	var new_gallery_chunks: Array[Moma] = []
	for gallery_chunk in gallery_chunks:
		var gallery_id := gallery_chunk.gallery_id
		if gallery_id < min_gallery_id or gallery_id > max_gallery_id:
			print("Despawning gallery with id ", gallery_id, " at ", gallery_chunk.position.x)
			remove_child(gallery_chunk)
		else:
			new_gallery_chunks.push_front(gallery_chunk)
	gallery_chunks = new_gallery_chunks

	# Spawn galleries that are near the player.
	for gallery_id: int in [min_gallery_id, middle_gallery_id, max_gallery_id]:
		var found := false
		for gallery_chunk in gallery_chunks:
			if gallery_chunk.gallery_id == gallery_id:
				found = true
				break
		if not found:
			var instance: Moma = gallery_chunk_scene.instantiate()
			instance.position.x = gallery_id * GALLERY_CHUNK_WIDTH
			print("Spawning new gallery with id ", gallery_id, " at ", instance.position.x)
			add_child(instance)
			gallery_chunks.push_front(instance)
			instance.init(gallery_id)


const SAVE_STATE_FILENAME := "user://save_state.json"

const AUTOSAVE_INTERVAL := 30.0

var seconds_since_last_save := 0.0

func save_state() -> void:
	var pos := player.global_position
	var state := {
		"player_position": [pos.x, pos.y, pos.z],
	}
	var file := FileAccess.open(SAVE_STATE_FILENAME, FileAccess.WRITE)
	var json_stringified := JSON.stringify(state)
	# print("Writing state: ", json_stringified)
	file.store_string(json_stringified)
	file.close()


func load_state() -> void:
	var pos: Vector3
	if FileAccess.file_exists(SAVE_STATE_FILENAME):
		var json_stringified := FileAccess.get_file_as_string(SAVE_STATE_FILENAME)
		# print("Reading state: ", json_stringified)
		var state = JSON.parse_string(json_stringified)
		var pos_array = state["player_position"]
		pos.x = pos_array[0]
		pos.y = pos_array[1]
		pos.z = pos_array[2]
	else:
		pos = player_start_position
	player.global_position = pos


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	load_state()
	sync_galleries()


func _process(delta) -> void:
	seconds_since_last_save += delta
	if seconds_since_last_save >= AUTOSAVE_INTERVAL:
		print("Autosaving.")
		save_state()
		seconds_since_last_save = 0.0
	sync_galleries()


func _notification(what):
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		print("Saving state on exit.")
		save_state()
