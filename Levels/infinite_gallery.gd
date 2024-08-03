class_name InfiniteGallery

extends Node3D


@export var gallery_chunk_scene: PackedScene

@export var player_scene: PackedScene

@onready var player_spawn_point: Node3D = %PlayerSpawnPoint

@onready var player_initial_teleport_point: Node3D = %PlayerInitialTeleportPoint

@onready var gallery_chunks: Array[Moma] = []

@onready var auto_saver: AutoSaver = $AutoSaver

@onready var world_environment: WorldEnvironment = %WorldEnvironment

## SDFGI is weird and we need to reset it once all our level geometry
## is loaded, which happens asynchronously in multiplayer. This keeps
## track of whether we've reset it yet.
var did_reset_lighting_in_multiplayer = false

## The width, along the x-axis, of the gallery chunk scene.
const GALLERY_CHUNK_WIDTH = 28

## Number of gallery chunks around each player to spawn to ensure that
## no player is ever looking into the abyss. Generally this should be
## 1, but it can be set to 0 for debugging.
const GALLERY_SPAWN_RADIUS = 1


func get_gallery_id(x: float) -> int:
	return floori(x / float(GALLERY_CHUNK_WIDTH))


## Respawn the galleries. This can be used if e.g. the database has
## been changed and we want to re-sync the world state with the db,
## without despawning the players.
func _respawn_galleries() -> void:
	# This will effectively abort all paintings currently being moved.
	for player in get_players():
		if player.moving_painting:
			player.moving_painting = null

	_despawn_all_galleries_except({})
	sync_galleries()


## Despawn all galleries except the ones whose IDs are keys in the given
## dictionary.
##
## Returns a dictionary whose keys are the IDs of galleries that were not
## despawned.
func _despawn_all_galleries_except(exceptions: Dictionary) -> Dictionary:
	var new_gallery_chunks: Array[Moma] = []
	var galleries_existing := {}
	for gallery_chunk in gallery_chunks:
		var gallery_id := gallery_chunk.gallery_id
		if !exceptions.has(gallery_id):
			print("Despawning gallery with id ", gallery_id, " at ", gallery_chunk.position.x)
			remove_child(gallery_chunk)
			gallery_chunk.queue_free()
		else:
			galleries_existing[gallery_id] = null
			new_gallery_chunks.push_back(gallery_chunk)
	gallery_chunks = new_gallery_chunks
	return galleries_existing


func get_players() -> Array[Player]:
	var players: Array[Player] = []
	for player in get_tree().get_nodes_in_group("Player"):
		players.push_back(player)
	return players


func _add_galleries_around_point(point: Vector3, galleries: Dictionary):
	var middle_gallery_id := get_gallery_id(point.x)
	var min_gallery_id := middle_gallery_id - GALLERY_SPAWN_RADIUS
	var max_gallery_id := middle_gallery_id + GALLERY_SPAWN_RADIUS
	for i in range(min_gallery_id, max_gallery_id + 1):
		galleries[i] = null


func sync_galleries() -> void:
	if Lobby.IS_CLIENT:
		# We should never be running this from the client.
		return

	var players := get_players()

	# We're going to take the union of all the galleries surrounding every player,
	# spawn them, and rely on Godot's MultiplayerSpawner/MultiplayerSynchronizer
	# to synchronize everything.
	#
	# Note that a big downside of this is that while it's fairly straightforward to
	# implement, performance degrades as this union grows in size. If there are 8
	# players in the same gallery, it should work OK, but if those players are
	# in totally different galleries, every client is going to synchronize with
	# at least 8 different galleries and all the paintings in them, which isn't
	# great.
	var galleries_to_exist := {}
	for player in players:
		_add_galleries_around_point(player.position, galleries_to_exist)
		_add_galleries_around_point(player.teleport_global_transform.origin, galleries_to_exist)

	# Get rid of galleries that are far from the players.
	var galleries_existing := _despawn_all_galleries_except(galleries_to_exist)

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
	# Even if we set the reference objects to not be visible, raycasts still intersect with
	# them, which is weird, so just remove them.
	%ForReferenceOnly.free()


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

	UserInterface.debug_draw_changed.connect(_on_debug_draw_changed)
	UserInterface.layout_config_container.new_layout_complete.connect(_on_new_layout_complete)
	UserInterface.global_illumination_changed.connect(reset_lighting)

	sync_galleries()


func _on_new_layout_complete():
	var players := get_players()
	if len(players) == 1:
		var player := players[0]

		# TODO: This might not work well in multiplayer, as the server doesn't have
		# authority on player rotation.
		player.global_rotation = player_spawn_point.global_rotation

		player.global_position = player_spawn_point.global_position
	_respawn_galleries()


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
	player.teleport_global_transform = player_initial_teleport_point.global_transform
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


func _on_debug_draw_changed(value: Viewport.DebugDraw):
	if value == Viewport.DEBUG_DRAW_DISABLED:
		# SDFGI gets weirdly tinted green when switching from some debug
		# draw modes to regular mode, so reset the lighting to make sure
		# that doesn't happen.
		reset_lighting()


func _should_disable_sdfgi() -> bool:
	return UserInterface.potato_mode or not UserInterface.global_illumination


func reset_lighting():
	if _should_disable_sdfgi():
		# Settings disable SDFGI, so no need to reset it.
		return

	print("Resetting SDFGI for proper lighting.")

	world_environment.environment.sdfgi_enabled = false
	for i in range(5):
		await get_tree().process_frame
		if not is_inside_tree():
			return
	if _should_disable_sdfgi():
		# Oof, the user changed settings while we were waiting! Don't enable
		# SDFGI after all.
		return
	world_environment.environment.sdfgi_enabled = true


func _on_multiplayer_spawner_spawned(_node: Node):
	if not did_reset_lighting_in_multiplayer:
		var galleries := get_tree().get_nodes_in_group("MomaGallery")
		var num_expected_galleries := GALLERY_SPAWN_RADIUS * 2 + 1
		if len(galleries) == num_expected_galleries:
			did_reset_lighting_in_multiplayer = true
			reset_lighting()
