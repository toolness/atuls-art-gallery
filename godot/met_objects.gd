extends Node


const ENABLE_MET_OBJECTS := true

const MAX_OBJECT_ATTEMPTS = 10


class MetObjectRecord:
	var object_id: int
	var title: String
	var date: String
	var width: float
	var height: float
	var small_image: String

	static func from_json(json_data: Variant) -> MetObjectRecord:
		var o := MetObjectRecord.new()
		o.object_id = json_data.object_id
		o.title = json_data.title
		o.date = json_data.date
		o.width = json_data.width / 100.0
		o.height = json_data.height / 100.0
		o.small_image = json_data.small_image
		return o

	func load_small_image() -> Image:
		var image := Image.load_from_file(small_image)
		image.generate_mipmaps()
		return image

	func load_small_image_texture() -> ImageTexture:
		return ImageTexture.create_from_image(load_small_image())

	func can_fit_in(max_width: float, max_height: float) -> bool:
		return width <= max_width and height <= max_height


var keyed_met_objects := {}

var unused_met_objects: Array[MetObjectRecord] = []


func _get_next_object() -> MetObjectRecord:
	RustMetObjects.next()
	var obj_str: String
	while not obj_str:
		# TODO: It's possible there are no more objects left, in which case we'll be
		# looping infinitely!
		obj_str = RustMetObjects.poll()
		await get_tree().process_frame
	return MetObjectRecord.from_json(JSON.parse_string(obj_str))


func _try_to_get_new_met_object(max_width: float, max_height: float) -> MetObjectRecord:
	for i in range(MAX_OBJECT_ATTEMPTS):
		var met_object := await _get_next_object()
		if met_object.can_fit_in(max_width, max_height):
			return met_object
		else:
			# The art is too wide/tall to fit on the wall.
			unused_met_objects.push_back(met_object)
	return null


func _try_to_get_unused_met_object(max_width: float, max_height: float) -> MetObjectRecord:
	for met_object in unused_met_objects:
		if met_object.can_fit_in(max_width, max_height):
			unused_met_objects.erase(met_object)
			return met_object
	return null


func try_to_get_next_object(key: String, max_width: float, max_height: float) -> MetObjectRecord:
	if not ENABLE_MET_OBJECTS:
		return null
	if not keyed_met_objects.has(key):
		var met_object := _try_to_get_unused_met_object(max_width, max_height)
		if not met_object:
			met_object = await _try_to_get_new_met_object(max_width, max_height)
			if not met_object:
				print("Unable to find object to fit in ", max_width, " x ", max_height)
				return null
		keyed_met_objects[key] = met_object
	return keyed_met_objects[key]


func _ready() -> void:
	pass
