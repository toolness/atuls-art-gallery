class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@export var player_scene: PackedScene

@onready var offline_mode_player: Player = $OfflineModePlayer

@onready var player_start_position: Vector3 = offline_mode_player.global_position

@onready var gallery_chunks: Array[Moma] = []

const GALLERY_CHUNK_WIDTH = 28

const GALLERY_SPAWN_RADIUS = 1


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


func sync_galleries() -> void:
	var player: Player
	if Lobby.IS_OFFLINE_MODE:
		player = offline_mode_player
	elif Lobby.IS_SERVER:
		# TODO: We will want to make sure galleries around _all_ players
		# are visible, not just one of them.
		player = get_tree().get_first_node_in_group("Player")

	if not player:
		# This saves system resources, but somewhat worryingly, weird stuff seems to happen if a single
		# player leaves the server and then rejoins--a bunch of errors about not being able to find the
		# multiplayer spawner are logged to the client. This seems to fix it.
		#
		# Note that errors do _not_ occur if a player logs off while another player is still connected, and
		# then the player rejoins. Very strange.
		for gallery_chunk in gallery_chunks:
			print("Despawning gallery with id ", gallery_chunk.gallery_id, " at ", gallery_chunk.position.x)
			gallery_chunk.queue_free()
		gallery_chunks = []
		return

	var middle_gallery_id := get_gallery_id(player.position.x)
	var min_gallery_id := middle_gallery_id - GALLERY_SPAWN_RADIUS
	var max_gallery_id := middle_gallery_id + GALLERY_SPAWN_RADIUS

	# Get rid of galleries that are far from the player.
	var new_gallery_chunks: Array[Moma] = []
	for gallery_chunk in gallery_chunks:
		var gallery_id := gallery_chunk.gallery_id
		if gallery_id < min_gallery_id or gallery_id > max_gallery_id:
			print("Despawning gallery with id ", gallery_id, " at ", gallery_chunk.position.x)
			gallery_chunk.queue_free()
		else:
			new_gallery_chunks.push_front(gallery_chunk)
	gallery_chunks = new_gallery_chunks

	# Spawn galleries that are near the player.
	for gallery_id: int in range(min_gallery_id, max_gallery_id + 1):
		var found := false
		for gallery_chunk in gallery_chunks:
			if gallery_chunk.gallery_id == gallery_id:
				found = true
				break
		if not found:
			var instance: Moma = gallery_chunk_scene.instantiate()
			instance.position.x = gallery_id * GALLERY_CHUNK_WIDTH
			print("Spawning new gallery with id ", gallery_id, " at ", instance.position.x)
			instance.init(gallery_id)
			add_child(instance)
			gallery_chunks.push_front(instance)
			instance.populate(player)


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	# Even if we set the reference gallery to not be visible, raycasts still intersect with
	# it, which is weird, so just remove it.
	%Moma_for_reference_only.queue_free()

	if not Lobby.IS_OFFLINE_MODE:
		offline_mode_player.free()

	if Lobby.IS_SERVER:
		multiplayer.peer_connected.connect(_on_peer_connected)
		multiplayer.peer_disconnected.connect(_on_peer_disconnected)
		# TODO: If we're not headless, spawn our own player.

	sync_galleries()


func _process(_delta) -> void:
	sync_galleries()


func _get_player_name(id: int) -> String:
	return "Player_" + str(id)


func _on_peer_connected(id: int):
	var player: Player = player_scene.instantiate()
	player.name = _get_player_name(id)
	player.peer_id = id
	add_child(player)
	player.global_position = player_start_position
	print("Spawned ", player.name, ".")


func _on_peer_disconnected(id: int):
	var player_name := _get_player_name(id)
	print("Despawning ", player_name, ".")
	var player: Player = get_node(player_name)
	if not player:
		print("Warning: ", player_name, " not found!")
		return
	player.queue_free()
