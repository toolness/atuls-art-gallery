class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

@export var painting_scene: PackedScene

const MIN_WALL_MOUNT_SIZE = 2

const MIN_CANVAS_SIZE = 0.5

const PAINTING_OFFSET = Vector3(0, -0.5, 0)

var gallery_id: int


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
		if aabb.size.x > MIN_WALL_MOUNT_SIZE:
			# We can mount art along the x-axis.
			width = aabb.size.x
			if normal.z < 0:
				y_rotation = PI
		elif aabb.size.z > MIN_WALL_MOUNT_SIZE:
			# We can mount art along the z-axis.
			width = aabb.size.z
			y_rotation = PI / 2
			if normal.x < 0:
				y_rotation += PI
		else:
			# This isn't a big enough wall to mount anything on.
			continue
		var painting: Painting = painting_scene.instantiate()
		if MetObjects.objects.size() > 0:
			var rand_idx := rng.randi_range(0, MetObjects.objects.size() - 1)
			painting.init_with_met_object(MetObjects.objects[rand_idx - 1])
		else:
			painting.init_with_size_and_color(
				rng.randf_range(MIN_CANVAS_SIZE, width / 2.0),
				rng.randf_range(MIN_CANVAS_SIZE, height / 1.5),
				Color(
					rng.randf_range(0.0, 1.0),
					rng.randf_range(0.0, 1.0),
					rng.randf_range(0.0, 1.0),
				)
			)
		add_child(painting)
		var painting_mount_point := mesh_instance.position + aabb.get_center() + PAINTING_OFFSET
		painting.translate(painting_mount_point)
		painting.rotate_y(y_rotation)
		# TODO: Use this width to spawn multiple paintings per wall.
		var _unused_width = width

		# Give the rest of the engine time to process the full frame, we're not in a rush and
		# processing all paintings synchronously will cause stutter.
		await get_tree().process_frame
		# TODO: It's unclear if we're going to continue if we've been removed from the scene
		# tree. If we are, then we should probably (somehow) check to see if we're still
		# in the scene tree before continuing.


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	print("Initializing gallery ", gallery_id)
	await populate_with_paintings()
