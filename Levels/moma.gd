class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

@onready var gallery_label: Label3D = $GalleryLabel

@export var painting_scene: PackedScene

const MIN_WALL_MOUNT_SIZE = 2

# The label is on the border between this gallery and
# the one next to it--let's make it be a label for the
# one next to it. This is because we start the player
# just beyond the edge of the gallery they start in,
# looking into the next gallery, and we want the
# gallery ID to reflect the gallery they're looking
# into.
const GALLERY_LABEL_ID_OFFSET = 1

const GALLERY_BASE_NAME = "MomaGallery_"

const GALLERY_PATTERN = "MomaGallery_*"

const PAINTING_BASE_NAME = "MomaPainting_"

const PAINTING_PATTERN = "MomaPainting_*"

## The gallery ID, synchronized by the server.
@export var gallery_id: int

static func try_to_find_painting_from_collision(collision: Object) -> Painting:
	if collision and collision is Node3D:
		var node: Node3D = collision
		var painting: Painting = node.find_parent(PAINTING_PATTERN)
		if painting is Painting:
			return painting
	return null


static func try_to_find_wall_from_collision(collision: Object) -> Wall:
	if collision and collision is Node3D:
		var node: Node3D = collision
		return Wall.try_from_object(node.get_parent_node_3d())
	return null


## This is only used on the server.
func make_painting(art_object: ArtObject) -> Painting:
	var painting: Painting = painting_scene.instantiate()
	painting.name = PAINTING_BASE_NAME + str(art_object.object_id)
	painting.init_with_art_object(art_object)
	add_child(painting)
	return painting


## This is only used on the server.
func place_art_object_on_wall(
	art_object: ArtObject,
	wall: Wall
) -> void:
	var painting := make_painting(art_object)
	var width_offset := wall.horizontal_direction * art_object.x
	var height_offset := Vector3.UP * art_object.y
	var painting_mount_point := wall.get_base_position() + width_offset + height_offset
	painting.translate(painting_mount_point)
	painting.rotate_y(wall.y_rotation)


class MovingPainting:
	var painting: Painting
	var offset: Vector3

	# These are only used on the server.
	var wall_x: float
	var wall_y: float
	var wall_id: String
	var gallery_id: int

	func finish_moving() -> void:
		painting.finish_interactive_placement()
		if not Lobby.IS_CLIENT:
			print("New painting position is object_id=", painting.art_object_id, " gallery_id=", gallery_id, " wall_id=", wall_id, " x=", wall_x, " y=", wall_y)
			ArtObjects.gallery_client.move_art_object(painting.art_object_id, gallery_id, wall_id, wall_x, wall_y)

	func _populate_wall_info(wall: Wall):
		var relative_position = painting.global_position - wall.get_global_base_position()
		wall_x = relative_position.dot(wall.horizontal_direction)
		wall_y = relative_position.y
		wall_id = wall.name
		gallery_id = wall.gallery.gallery_id

	func move_along_wall(raycast: RayCast3D) -> void:
		# Note: if we're in a multiplayer situation and this is the client, we'll see jittering.
		# I think this is becuase the server will be sending updates to change these things too,
		# only it will be from the past due to latency, resulting in "fighting" between client
		# and server values.
		var wall := Moma.try_to_find_wall_from_collision(raycast.get_collider())
		if not wall:
			return
		var point := raycast.get_collision_point()
		# TODO: Don't move the painting if it's:
		#   * hanging off the edge of the wall
		#   * intersecting with another painting
		painting.global_position = point - offset.rotated(Vector3.UP, wall.y_rotation)
		painting.rotation = Vector3.ZERO
		painting.rotate_y(wall.y_rotation)
		var parent := painting.get_parent()
		if parent != wall.gallery:
			print("Moving painting from ", parent.name, " to ", wall.gallery.name)
			parent.remove_child(painting)
			wall.gallery.add_child(painting)
		_populate_wall_info(wall)

	static func try_to_start_moving(raycast: RayCast3D) -> MovingPainting:
		var _painting := Moma.try_to_find_painting_from_collision(raycast.get_collider())
		if not _painting:
			return null
		_painting.start_interactive_placement()
		# The painting's collider is disabled, so the raycast won't hit it now.
		raycast.force_raycast_update()
		var point := raycast.get_collision_point()
		var wall := Moma.try_to_find_wall_from_collision(raycast.get_collider())
		if not wall:
			# This is unusual, as the painting *should* have been right in front of a wall.
			# It might be the case that the user is at the very edge of the raycast's max distance,
			# such that the raycast can hit the painting but not the wall behind it.
			_painting.finish_interactive_placement()
			return
		var moving_painting := MovingPainting.new()
		moving_painting.painting = _painting
		moving_painting.offset = (point - _painting.global_position).rotated(Vector3.UP, -wall.y_rotation)
		moving_painting._populate_wall_info(wall)
		return moving_painting


class Wall:
	var name: String
	var width: float
	var height: float
	var y_rotation: float
	var horizontal_direction: Vector3
	var mesh_instance: MeshInstance3D
	var aabb: AABB
	var normal: Vector3
	var gallery: Moma

	func _try_to_configure(object: Object, is_importing: bool) -> bool:
		if not is_instance_of(object, MeshInstance3D):
			return false
		mesh_instance = object
		name = mesh_instance.name
		aabb = mesh_instance.get_aabb()
		height = aabb.size.y
		if height < MIN_WALL_MOUNT_SIZE:
			# This is either a floor or ceiling, or it's just a wall
			# that isn't tall enough for our needs.
			return false
		var faces := mesh_instance.mesh.get_faces()
		if faces.size() != 6:
			# This isn't a plane.
			return false
		var first := faces[1] - faces[0]
		var second := faces[2] - faces[0]
		normal = second.cross(first).normalized()
		if aabb.size.x > MIN_WALL_MOUNT_SIZE:
			# We can mount art along the x-axis.
			width = aabb.size.x
			horizontal_direction = Vector3.RIGHT
			if normal.z < 0:
				y_rotation = PI
		elif aabb.size.z > MIN_WALL_MOUNT_SIZE:
			# We can mount art along the z-axis.
			width = aabb.size.z
			horizontal_direction = Vector3.BACK
			y_rotation = PI / 2
			if normal.x < 0:
				y_rotation += PI
		else:
			# This isn't a big enough wall to mount anything on.
			return false
		if not is_importing:
			gallery = mesh_instance.find_parent(GALLERY_PATTERN)
			assert(gallery is Moma)
		return true

	func get_base_position() -> Vector3:
		return mesh_instance.position + mesh_instance.get_aabb().position

	func get_global_base_position() -> Vector3:
		return mesh_instance.global_position + mesh_instance.get_aabb().position

	static func try_from_object(object: Object, is_importing: bool = false) -> Wall:
		var wall: Wall = Wall.new()
		if not wall._try_to_configure(object, is_importing):
			return null
		return wall


func sort_walls_by_distance_from_player(walls: Array[Wall], player: Player):
	var wall_distances_from_player := {}
	for wall in walls:
		var wall_pos := wall.mesh_instance.global_position + wall.aabb.get_center()
		var distance_from_player = wall_pos.distance_to(player.global_position)
		wall_distances_from_player[wall] = distance_from_player
	var sort_by_distance_from_player := func is_b_after_a(a: Wall, b: Wall) -> bool:
		var a_dist: float = wall_distances_from_player[a]
		var b_dist: float = wall_distances_from_player[b]
		return b_dist > a_dist
	walls.sort_custom(sort_by_distance_from_player)


func _get_walls() -> Array[Wall]:
	var walls: Array[Wall] = []

	for child in gallery.get_children():
		var wall := Wall.try_from_object(child)
		if wall:
			walls.push_back(wall)

	return walls


## This is only used on the server.
func populate_with_paintings(players: Array[Player]) -> int:
	var count := 0

	var moving_painting_ids := {}
	for player in players:
		if player.moving_painting:
			moving_painting_ids[player.moving_painting.painting.art_object_id] = null

	var walls := _get_walls()

	if len(players) == 1:
		# Optimization for single player mode: populate the closest walls first.
		sort_walls_by_distance_from_player(walls, players[0])

	for wall in walls:
		var art_objects := await ArtObjects.get_art_objects_for_gallery_wall(gallery_id, wall.name)
		if not is_inside_tree():
			return count
		for art_object in art_objects:
			if moving_painting_ids.has(art_object.object_id):
				# A player is currently moving this painting, don't spawn it.
				print("Not spawning ", art_object.object_id, " because it is being moved by player.")
				continue
			# print(gallery_id, " ", child.name, " ", art_object.title, " ", art_object.x, " ", art_object.y)
			place_art_object_on_wall(art_object, wall)
			count += 1
			# Give the rest of the engine time to process the full frame, we're not in a rush and
			# processing all paintings synchronously will cause stutter.
			await _let_engine_breathe_between_painting_spawns()
			if not is_inside_tree():
				# We've been removed from the scene tree, bail.
				return count
	return count


func _let_engine_breathe_between_painting_spawns():
	var frame_count := 1
	if Lobby.IS_SERVER:
		# If we're the server, we want to wait _extra_ long. This is because
		# network latency is going to cause clients to get lots of spawns at
		# once, effectively causing them to spawn everything in a single frame,
		# so we're going to really take our time here to ensure that they
		# don't get overwhelmed and stutter.
		frame_count = 10
	for i in range(frame_count):
		await get_tree().process_frame
		if not is_inside_tree():
			return


func init(new_gallery_id: int):
	gallery_id = new_gallery_id
	name = GALLERY_BASE_NAME + str(gallery_id)


func _ready():
	gallery_label.text = str(gallery_id + GALLERY_LABEL_ID_OFFSET)
	_paint_gallery_walls()


const WALL_SURFACE_IDX = 0

## This is Farrow & Ball's "Manor House Gray".
const MANOR_HOUSE_GRAY = Color(158.0 / 255.0, 160.0 / 255.0, 157.0 / 255.0)

func get_gallery_wall_color() -> Color:
	if gallery_id < 0:
		# Private collection.
		return MANOR_HOUSE_GRAY
	# Temporary exhibition.
	return Color.WHITE

func _paint_gallery_walls():
	var first_wall := _get_walls()[0]
	var wall_material: StandardMaterial3D = first_wall.mesh_instance.mesh.surface_get_material(WALL_SURFACE_IDX)
	var override_material: StandardMaterial3D = wall_material.duplicate()
	override_material.albedo_color = get_gallery_wall_color()

	# We need to paint all surfaces that use the same material as the walls.
	# This includes not _just_ walls--for instance, passageways need to be painted.
	for child in gallery.get_children():
		if not is_instance_of(child, MeshInstance3D):
			continue
		var mesh_instance_child: MeshInstance3D = child
		var mesh := mesh_instance_child.mesh
		var num_surfaces := mesh.get_surface_count()
		for i in range(num_surfaces):
			var material: StandardMaterial3D = mesh.surface_get_material(i)
			if material == wall_material:
				# This uses the same material as our walls, so paint it.
				mesh_instance_child.set_surface_override_material(i, override_material)


## This is only used on the server.
func populate(players: Array[Player]) -> void:
	print("Initializing gallery ", gallery_id)
	var count := await populate_with_paintings(players)
	print("Populated gallery ", gallery_id, " with ", count, " paintings.")
