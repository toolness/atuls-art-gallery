class_name Painting

extends Node3D


const PAINTING_SURFACE_IDX = 1


var met_object: MetObjects.MetObjectRecord


func configure_wall_label(painting_width: float, painting_height: float, text: String) -> void:
	var wall_label: Label3D = $wall_label
	var wall_label_y_offset := -wall_label.position.y - 0.5
	wall_label.position.y = -((painting_height / 2) + wall_label_y_offset)
	wall_label.position.x = -(painting_width / 2)
	wall_label.text = text


func init_with_size_and_color(width: float, height: float, color: Color) -> void:
	configure_wall_label(width, height, "#" + color.to_html(false).to_upper() + "\n2024")
	var painting: MeshInstance3D  = $painting.get_child(0)
	painting.set_scale(Vector3(width, height, 1.0))
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	var duplicate_material: StandardMaterial3D = material.duplicate()
	duplicate_material.albedo_color = color
	painting.set_surface_override_material(PAINTING_SURFACE_IDX, duplicate_material)


func init_with_met_object(object: MetObjects.MetObjectRecord) -> void:
	met_object = object
	configure_wall_label(object.width, object.height, object.title + "\n" + object.date)
	var painting: MeshInstance3D  = $painting.get_child(0)
	painting.set_scale(Vector3(object.width, object.height, 1.0))
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	var duplicate_material: StandardMaterial3D = material.duplicate()
	duplicate_material.albedo_color = Color.TRANSPARENT
	duplicate_material.albedo_texture = object.load_small_image_texture()
	painting.set_surface_override_material(PAINTING_SURFACE_IDX, duplicate_material)
