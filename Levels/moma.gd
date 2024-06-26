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

var gallery_id: int

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


func make_painting(met_object: MetObject) -> Painting:
	var painting: Painting = painting_scene.instantiate()
	painting.name = PAINTING_BASE_NAME + str(met_object.object_id)
	painting.init_with_met_object(met_object)
	add_child(painting)
	return painting


func place_met_object_on_wall(
	met_object: MetObject,
	wall: Wall,
	image: Image
) -> void:
	var painting := make_painting(met_object)
	painting.paint_and_resize(image)
	var width_offset := wall.horizontal_direction * met_object.x
	var height_offset := Vector3.UP * met_object.y
	var painting_mount_point := wall.get_base_position() + width_offset + height_offset
	painting.translate(painting_mount_point)
	painting.rotate_y(wall.y_rotation)


class MovingPainting:
	var painting: Painting
	var offset: Vector3
	var wall_x: float
	var wall_y: float
	var wall_id: String
	var gallery_id: int

	func finish_moving() -> void:
		painting.finish_interactive_placement()
		print("New painting position is object_id=", painting.met_object.object_id, " gallery_id=", gallery_id, " wall_id=", wall_id, " x=", wall_x, " y=", wall_y)
		MetObjects.gallery_client.move_met_object(painting.met_object.object_id, gallery_id, wall_id, wall_x, wall_y)

	func _populate_wall_info(wall: Wall):
		var relative_position = painting.global_position - wall.get_global_base_position()
		wall_x = relative_position.dot(wall.horizontal_direction)
		wall_y = relative_position.y
		wall_id = wall.name
		gallery_id = wall.gallery.gallery_id

	func move_along_wall(raycast: RayCast3D) -> void:
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

	func _try_to_configure(object: Object) -> bool:
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
		gallery = mesh_instance.find_parent(GALLERY_PATTERN)
		assert(gallery is Moma)
		return true

	func get_base_position() -> Vector3:
		return mesh_instance.position + mesh_instance.get_aabb().position

	func get_global_base_position() -> Vector3:
		return mesh_instance.global_position + mesh_instance.get_aabb().position

	static func try_from_object(object: Object) -> Wall:
		var wall: Wall = Wall.new()
		if not wall._try_to_configure(object):
			return null
		return wall


func populate_with_paintings(player: Player) -> int:
	var count := 0
	var walls: Array[Wall] = []
	# This is a mapping from walls to their distances to the player.
	var wall_distances_from_player := {}

	for child in gallery.get_children():
		var wall := Wall.try_from_object(child)
		if wall:
			walls.push_back(wall)
			var wall_pos := wall.mesh_instance.global_position + wall.aabb.get_center()
			var distance_from_player = wall_pos.distance_to(player.global_position)
			wall_distances_from_player[wall] = distance_from_player

	var sort_by_distance_from_player := func is_b_after_a(a: Wall, b: Wall) -> bool:
		var a_dist: float = wall_distances_from_player[a]
		var b_dist: float = wall_distances_from_player[b]
		return b_dist > a_dist

	walls.sort_custom(sort_by_distance_from_player)

	for wall in walls:
		var met_objects := await MetObjects.get_met_objects_for_gallery_wall(gallery_id, wall.name)
		if not is_inside_tree():
			return count
		for met_object in met_objects:
			if player.moving_painting and player.moving_painting.painting.met_object.object_id == met_object.object_id:
				# The player is currently moving this painting, don't spawn it.
				print("Not spawning ", met_object.object_id, " because it is being moved by player.")
				continue
			# print(gallery_id, " ", child.name, " ", met_object.title, " ", met_object.x, " ", met_object.y)
			var image := await MetObjects.fetch_small_image(met_object.object_id)
			if not is_inside_tree():
				# We despawned, exit.
				return count
			if not image:
				# Oof, fetching the image failed.
				continue
			place_met_object_on_wall(met_object, wall, image)
			count += 1
			# Give the rest of the engine time to process the full frame, we're not in a rush and
			# processing all paintings synchronously will cause stutter.
			await get_tree().process_frame
			if not is_inside_tree():
				# We've been removed from the scene tree, bail.
				return count
	return count


func init(new_gallery_id: int):
	gallery_id = new_gallery_id
	name = GALLERY_BASE_NAME + str(gallery_id)


func populate(player: Player) -> void:
	gallery_label.text = str(gallery_id + GALLERY_LABEL_ID_OFFSET)
	print("Initializing gallery ", gallery_id)
	var count := await populate_with_paintings(player)
	print("Populated gallery ", gallery_id, " with ", count, " paintings.")
