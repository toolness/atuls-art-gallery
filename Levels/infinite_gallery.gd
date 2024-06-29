class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@export var player_scene: PackedScene

@onready var player_spawn_point: Node3D = %PlayerSpawnPoint

@onready var gallery_chunks: Array[Moma] = []

@onready var auto_saver: AutoSaver = $AutoSaver

@onready var world_environment: WorldEnvironment = %WorldEnvironment

var did_reset_lighting = false

## The width, along the x-axis, of the gallery chunk scene.
const GALLERY_CHUNK_WIDTH = 28

## Number of gallery chunks around each player to spawn to ensure that
## no player is ever looking into the abyss. Generally this should be
## 1, but it can be set to 0 for debugging.
const GALLERY_SPAWN_RADIUS = 1


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


func sync_galleries() -> void:
	if Lobby.IS_CLIENT:
		# We should never be running this from the client.
		return

	var players: Array[Player] = []
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
			instance.populate(players)


func _enter_tree():
	# Even if we set the reference gallery to not be visible, raycasts still intersect with
	# it, which is weird, so just remove it.
	%Moma_for_reference_only.free()


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	Lobby.start()
	if Lobby.IS_SERVER:
		multiplayer.peer_connected.connect(_on_peer_connected)
		multiplayer.peer_disconnected.connect(_on_peer_disconnected)
		if not Lobby.IS_HEADLESS:
			# Spawn the player running the server.
			_on_peer_connected(1)
	elif Lobby.IS_OFFLINE_MODE:
		var player := _spawn_player(1)
		auto_saver.init(player)

	sync_galleries()


func _process(_delta) -> void:
	sync_galleries()


func _get_player_name(id: int) -> String:
	return "Player_" + str(id)


func _spawn_player(id: int) -> Player:
	var player: Player = player_scene.instantiate()
	player.name = _get_player_name(id)
	player.peer_id = id
	player.initial_rotation = player_spawn_point.global_rotation
	add_child(player)
	player.global_position = player_spawn_point.global_position
	print("Spawned ", player.name, ".")
	return player


func _on_peer_connected(id: int):
	_spawn_player(id)


func _on_peer_disconnected(id: int):
	var player_name := _get_player_name(id)
	print("Despawning ", player_name, ".")
	var player: Player = get_node(player_name)
	if not player:
		print("Warning: ", player_name, " not found!")
		return
	player.queue_free()


func _on_multiplayer_spawner_spawned(_node: Node):
	if not did_reset_lighting:
		var galleries := get_tree().get_nodes_in_group("MomaGallery")
		var num_expected_galleries := GALLERY_SPAWN_RADIUS * 2 + 1
		if len(galleries) == num_expected_galleries:
			print("All galleries loaded, resetting SDFGI for proper lighting.")
			did_reset_lighting = true
			world_environment.environment.sdfgi_enabled = false
			for i in range(5):
				await get_tree().process_frame
				if not is_inside_tree():
					return
			world_environment.environment.sdfgi_enabled = true
