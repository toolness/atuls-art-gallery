extends Node


const MAX_REQUESTS_PER_FRAME = 10

const NULL_REQUEST_ID = 0

var requests = {}


class MetObjectsRequest:
	var response: Array[MetObject]
	signal responded


class ImageRequest:
	var response: Image
	signal responded


func fetch_small_image(object_id: int) -> Image:
	var request := ImageRequest.new()
	var request_id := gallery_client.fetch_small_image(object_id)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return null
	requests[request_id] = request
	await request.responded
	return request.response


func get_met_objects_for_gallery_wall(gallery_id: int, wall_id: String) -> Array[MetObject]:
	var request := MetObjectsRequest.new()
	var request_id := gallery_client.get_met_objects_for_gallery_wall(gallery_id, wall_id)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return []
	requests[request_id] = request
	await request.responded
	return request.response


var fatal_error_message: String

var gallery_client: GalleryClient


func crash(message: String):
	OS.alert(message)
	OS.crash(message)


func copy_initial_db() -> void:
	const GALLERY_DB_PATH = "user://gallery.sqlite"
	if not FileAccess.file_exists(GALLERY_DB_PATH):
		const INITIAL_DB_PATH = "res://initial-db.sqlite"
		print("Copying initial DB to ", GALLERY_DB_PATH, ".")
		# I'd love to use DirAccess.copy_absolute() here because it
		# probably streams things, but it can't open the file and
		# basically seems to be completely broken:
		#
		#   https://github.com/godotengine/godot/issues/74105
		#
		# For now, at least, the initial DB isn't so big that it
		# will exhaust system memory, so just read the whole damn
		# thing into memory and write it.
		#
		# Beyond that, I don't really understand why I can't just
		# put this file alongside all the other files in the exported
		# project, rather than having to stuff it in the PCK/ZIP and
		# then extract it, but that doesn't seem to be something Godot
		# easily supports.
		var data := FileAccess.get_file_as_bytes(INITIAL_DB_PATH)
		print("Read initial db ", data.size())
		if data.size() == 0:
			crash("Could not open initial DB!")
			return
		var out_file := FileAccess.open(GALLERY_DB_PATH, FileAccess.WRITE)
		if not out_file:
			crash("Unable to write initial DB!")
		out_file.store_buffer(data)
		out_file.close()
		print("Wrote initial DB.")


func _ready() -> void:
	if not OS.has_feature("editor"):
		copy_initial_db()
	gallery_client = GalleryClient.new()
	gallery_client.connect()


func _process(_delta) -> void:
	if fatal_error_message:
		return
	fatal_error_message = gallery_client.take_fatal_error()
	if fatal_error_message:
		UserInterface.show_fatal_error(fatal_error_message)
		# TODO: It would be nice to let all requests know that an error occurred.
		requests.clear()
		return
	for i in range(MAX_REQUESTS_PER_FRAME):
		var obj := gallery_client.poll()
		if not obj:
			return
		if not requests.has(obj.request_id):
			print("Warning: request #", obj.request_id, " does not exist.")
			return
		var request = requests[obj.request_id]
		requests.erase(obj.request_id)
		if request is ImageRequest:
			var r: ImageRequest = request
			r.response = obj.take_optional_image()
			r.responded.emit()
		elif request is MetObjectsRequest:
			var r: MetObjectsRequest = request
			r.response = obj.take_met_objects()
			r.responded.emit()
		else:
			assert(false, "Unknown request type, cannot fill response")
