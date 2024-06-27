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
	if Lobby.IS_CLIENT:
		# We should never be running this from the client.
		return

	var players: Array[Player] = []
	if Lobby.IS_OFFLINE_MODE:
		players = [offline_mode_player]
	elif Lobby.IS_SERVER:
		for player in get_tree().get_nodes_in_group("Player"):
			players.push_back(player)

	var galleries_to_exist := {}
	for player in players:
		var middle_gallery_id := get_gallery_id(player.position.x)
		var min_gallery_id := middle_gallery_id - GALLERY_SPAWN_RADIUS
		var max_gallery_id := middle_gallery_id + GALLERY_SPAWN_RADIUS
		for i in range(min_gallery_id, max_gallery_id + 1):
			galleries_to_exist[i] = null

	# Get rid of galleries that are far from the players.
	var new_gallery_chunks: Array[Moma] = []
	var galleries_existing := {}
	for gallery_chunk in gallery_chunks:
		var gallery_id := gallery_chunk.gallery_id
		if !galleries_to_exist.has(gallery_id):
			print("Despawning gallery with id ", gallery_id, " at ", gallery_chunk.position.x)
			gallery_chunk.queue_free()
		else:
			galleries_existing[gallery_id] = null
			new_gallery_chunks.push_back(gallery_chunk)
	gallery_chunks = new_gallery_chunks

	# Spawn galleries that are near the players.
	for gallery_id: int in galleries_to_exist.keys():
		if not galleries_existing.has(gallery_id):
			var instance: Moma = gallery_chunk_scene.instantiate()
			instance.position.x = gallery_id * GALLERY_CHUNK_WIDTH
			print("Spawning new gallery with id ", gallery_id, " at ", instance.position.x)
			instance.init(gallery_id)
			add_child(instance)
			gallery_chunks.push_front(instance)
			# TODO: Pass in all players
			instance.populate(players[0])


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
		if not Lobby.IS_HEADLESS:
			# Spawn the player running the server.
			_on_peer_connected(1)

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
