class_name Painting

extends Node3D


const PAINTING_SURFACE_IDX = 1


var met_object: MetObject

var painting_surface_material: StandardMaterial3D

var original_albedo_color: Color

@onready var painting: MeshInstance3D = $painting/Painting

@onready var collision_shape: CollisionShape3D = $painting/Painting/StaticBody3D/CollisionShape3D

@onready var wall_label: Label3D = $wall_label

@export var inner_painting_scale: Vector3

@export var met_object_id: int

func _ready():
	if not Lobby.IS_OFFLINE_MODE:
		if inner_painting_scale:
			painting.set_scale(inner_painting_scale)
		else:
			print("Warning: No inner_painting_scale available for painting!")
		if met_object_id:
			var image := await MetObjects.fetch_small_image(met_object_id)
			if not is_inside_tree():
				# We despawned, exit.
				return
			if not image:
				# Oof, fetching the image failed.
				visible = false
				return
			set_image(image)
		else:
			print("Warning: No met_object_id available for painting!")


func _get_side_multiplier(value: float) -> float:
	if value == 0.0:
		return 0.0
	elif value < 0.0:
		return -1.0
	return 1.0


func configure_wall_label(painting_width: float, painting_height: float, text: String) -> void:
	var aabb_size := painting.get_aabb().size
	var x := wall_label.position.x
	var wall_label_x_offset := absf(x) - aabb_size.x / 2
	wall_label.position.x = _get_side_multiplier(x) * (painting_width / 2 + wall_label_x_offset)
	var y := wall_label.position.y
	var wall_label_y_offset := absf(y) - aabb_size.y / 2
	wall_label.position.y = _get_side_multiplier(y) * (painting_height / 2 + wall_label_y_offset)
	wall_label.text = text


func init_with_met_object(object: MetObject):
	met_object = object
	inner_painting_scale = Vector3(object.width, object.height, 1.0)
	met_object_id = met_object.object_id


func paint_and_resize(image: Image) -> void:
	configure_wall_label(met_object.width, met_object.height, met_object.title + "\n" + met_object.date)
	painting.set_scale(inner_painting_scale)
	set_image(image)


func set_image(image: Image):
	image.generate_mipmaps()
	var texture := ImageTexture.create_from_image(image)
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	painting_surface_material = material.duplicate()
	painting_surface_material.albedo_color = Color.TRANSPARENT
	painting_surface_material.albedo_texture = texture
	painting.set_surface_override_material(PAINTING_SURFACE_IDX, painting_surface_material)


func try_to_open_in_browser():
	# TODO: The conditional is from when paintings could potentially have
	# solid colors instead of met objects, consider removing it and renaming
	# the function to `open_in_browser`... Or just get rid of it and have clients
	# directly reference `met_object`.
	if met_object:
		met_object.open_in_browser()


func start_interactive_placement():
	original_albedo_color = painting_surface_material.albedo_color
	painting_surface_material.albedo_color = Color.GREEN
	collision_shape.disabled = true


func finish_interactive_placement():
	painting_surface_material.albedo_color = original_albedo_color
	collision_shape.disabled = false
