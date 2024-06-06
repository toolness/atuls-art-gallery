class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

@onready var gallery_label: Label3D = $GalleryLabel

@export var painting_scene: PackedScene

const MIN_WALL_MOUNT_SIZE = 2

const MIN_CANVAS_SIZE = 0.5

const PAINTING_Y_OFFSET = -0.5

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


func make_painting() -> Painting:
	var painting: Painting = painting_scene.instantiate()
	latest_painting_id += 1
	painting.name = PAINTING_BASE_NAME + str(latest_painting_id)
	add_child(painting)
	return painting


func place_paintings_along_wall(
	key: String,
	rng: RandomNumberGenerator,
	base_position: Vector3,
	width: float,
	height: float,
	y_rotation: float,
	horizontal_direction: Vector3,
) -> void:
	var painting: Painting
	var painting_width: float
	var met_object := await MetObjects.try_to_get_next_object(key, width, height)
	if met_object:
		painting = make_painting()
		painting_width = met_object.width
		painting.init_with_met_object(met_object)
	elif not MetObjects.ENABLE_MET_OBJECTS:
		painting = make_painting()
		painting_width = rng.randf_range(MIN_CANVAS_SIZE, width / 2.0)
		painting.init_with_size_and_color(
			painting_width,
			rng.randf_range(MIN_CANVAS_SIZE, height / 1.5),
			Color(
				rng.randf_range(0.0, 1.0),
				rng.randf_range(0.0, 1.0),
				rng.randf_range(0.0, 1.0),
			)
		)
	else:
		return
	var width_offset := horizontal_direction * (width / 2.0)
	var height_offset := ((height / 2.0) + PAINTING_Y_OFFSET)
	var painting_mount_point := base_position + width_offset + Vector3.UP * height_offset
	painting.translate(painting_mount_point)
	painting.rotate_y(y_rotation)

	var margin_width := width / 2.0 - painting_width / 2.0
	if margin_width > MIN_WALL_MOUNT_SIZE:
		# Place paintings between the beginning of the wall and the start of the painting.
		await place_paintings_along_wall(key + "_l", rng, base_position, margin_width, height, y_rotation, horizontal_direction)
		# Place paintings between the end of the wall and the end of the painting.
		var end_base_position := base_position + (horizontal_direction * (width / 2.0 + painting_width / 2.0))
		await place_paintings_along_wall(key + "_r", rng, end_base_position, margin_width, height, y_rotation, horizontal_direction)

	# Give the rest of the engine time to process the full frame, we're not in a rush and
	# processing all paintings synchronously will cause stutter.
	var tree := get_tree()
	if not tree:
		# We've been removed from the scene tree, bail.
		return
	await tree.process_frame


func populate_with_paintings() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(gallery_id)
	for child in gallery.get_children():
		if not is_instance_of(child, MeshInstance3D):
			continue
		var mesh_instance: MeshInstance3D = child
		var aabb := mesh_instance.get_aabb()
		var height := aabb.size.y
		if height < MIN_WALL_MOUNT_SIZE:
			# This is either a floor or ceiling, or it's just a wall
			# that isn't tall enough for our needs.
			continue
		var faces := mesh_instance.mesh.get_faces()
		if faces.size() != 6:
			# This isn't a plane.
			continue
		var first := faces[1] - faces[0]
		var second := faces[2] - faces[0]
		var normal := second.cross(first).normalized()
		var width: float
		var y_rotation: float
		var horizontal_direction: Vector3
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
			continue
		await place_paintings_along_wall(
			str(gallery_id) + "_" + child.name,
			rng,
			mesh_instance.position + aabb.position,
			width,
			height,
			y_rotation,
			horizontal_direction,
		)


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	gallery_label.text = str(gallery_id + GALLERY_LABEL_ID_OFFSET)
	print("Initializing gallery ", gallery_id)
	await populate_with_paintings()
