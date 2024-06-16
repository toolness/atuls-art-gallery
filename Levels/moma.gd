class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

@onready var gallery_label: Label3D = $GalleryLabel

@export var painting_scene: PackedScene

const MIN_WALL_MOUNT_SIZE = 2

const GALLERY_LABEL_ID_OFFSET = 101

const PAINTING_BASE_NAME = "MomaPainting"

const PAINTING_PATTERN = "MomaPainting*"

var gallery_id: int

var latest_painting_id := 0

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


func make_painting() -> Painting:
	var painting: Painting = painting_scene.instantiate()
	latest_painting_id += 1
	painting.name = PAINTING_BASE_NAME + str(latest_painting_id)
	add_child(painting)
	return painting


func place_met_object_on_wall(
	met_object: MetObject,
	wall: Wall
) -> void:
	var painting := make_painting()
	painting.init_with_met_object(met_object)
	var width_offset := wall.horizontal_direction * met_object.x
	var height_offset := Vector3.UP * met_object.y
	var painting_mount_point := wall.get_base_position() + width_offset + height_offset
	painting.translate(painting_mount_point)
	painting.rotate_y(wall.y_rotation)


class MovingPainting:
	var painting: Painting
	var offset: Vector3

	func finish_moving() -> void:
		painting.finish_interactive_placement()

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
		return moving_painting


class Wall:
	var width: float
	var height: float
	var y_rotation: float
	var horizontal_direction: Vector3
	var mesh_instance: MeshInstance3D
	var aabb: AABB
	var normal: Vector3

	func _try_to_configure(object: Object) -> bool:
		if not is_instance_of(object, MeshInstance3D):
			return false
		mesh_instance = object
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
		return true

	func get_base_position() -> Vector3:
		return mesh_instance.position + mesh_instance.get_aabb().position

	static func try_from_object(object: Object) -> Wall:
		var wall: Wall = Wall.new()
		if not wall._try_to_configure(object):
			return null
		return wall


func populate_with_paintings() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(gallery_id)
	for child in gallery.get_children():
		var wall := Wall.try_from_object(child)
		if not wall:
			continue

		var met_objects := await MetObjects.get_met_objects_for_gallery_wall(gallery_id, child.name)
		if not is_inside_tree():
			return
		for met_object in met_objects:
			print(gallery_id, " ", child.name, " ", met_object.title, " ", met_object.x, " ", met_object.y)
			place_met_object_on_wall(met_object, wall)
			# Give the rest of the engine time to process the full frame, we're not in a rush and
			# processing all paintings synchronously will cause stutter.
			if not is_inside_tree():
				# We've been removed from the scene tree, bail.
				return
			await get_tree().process_frame


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	gallery_label.text = str(gallery_id + GALLERY_LABEL_ID_OFFSET)
	print("Initializing gallery ", gallery_id)
	await populate_with_paintings()
