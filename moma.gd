class_name Moma

extends Node3D

@onready var gallery: Node3D = $gallery

@export var painting_scene: PackedScene

const MIN_WALL_MOUNT_SIZE = 2

var gallery_id: int

# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	for child in gallery.get_children():
		if is_instance_of(child, MeshInstance3D):
			var mesh_instance: MeshInstance3D = child
			var aabb: AABB = mesh_instance.get_aabb()
			var height = aabb.size.y
			if height < MIN_WALL_MOUNT_SIZE:
				# This is either a floor or ceiling, or it's just a wall
				# that isn't tall enough for our needs.
				continue
			var faces = mesh_instance.mesh.get_faces()
			if faces.size() != 6:
				# This isn't a plane.
				continue
			var first: Vector3 = faces[1] - faces[0]
			var second: Vector3 = faces[2] - faces[0]
			var normal = second.cross(first).normalized()
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
			# TODO: Load images dynamically with:
			# https://docs.godotengine.org/en/stable/tutorials/io/runtime_file_loading_and_saving.html
			var painting: Painting = painting_scene.instantiate()
			add_child(painting)
			var painting_mount_point: Vector3 = mesh_instance.position + aabb.get_center()
			painting.translate(painting_mount_point)
			painting.rotate_y(y_rotation)
			# TODO: Use this width to spawn multiple paintings per wall.
			var _unused_width = width
			# print("COOL ", child.name, " ", normal, " ", width, " ", painting_mount_point)


func init(new_gallery_id: int) -> void:
	gallery_id = new_gallery_id
	print("Initializing gallery ", gallery_id)
