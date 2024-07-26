extends Node


const NULL_REQUEST_ID = 0

## Try to only spend these many microseconds per frame processing
## responses from the Rust side, as we don't want to skip frames.
const MAX_USEC_PER_FRAME = 8000

## If we spend more than these many microseconds processing
## responses from the Rust side, log a warning.
const WARNING_USEC_PER_FRAME = 16000


var requests = {}


class ArtObjectsRequest:
	var response: Array[ArtObject]
	signal responded


class ImageRequest:
	var response: Image
	signal responded


class EmptyRequest:
	signal responded


class IntRequest:
	var response: int
	signal responded


func _fetch_image(object_id: int, size: String) -> Image:
	if Lobby.IS_HEADLESS:
		return Image.create(1, 1, false, Image.FORMAT_L8)
	var request := ImageRequest.new()
	var request_id: int
	if size == "small":
		request_id = gallery_client.fetch_small_image(object_id)
	elif size == "large":
		request_id = gallery_client.fetch_large_image(object_id)
	else:
		crash("Invalid image size: " +  size)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return null
	requests[request_id] = request
	await request.responded
	return request.response


func fetch_small_image(object_id: int) -> Image:
	return await _fetch_image(object_id, "small")


func fetch_large_image(object_id: int) -> Image:
	return await _fetch_image(object_id, "large")


func count_art_objects(filter: String) -> int:
	var request := IntRequest.new()
	var request_id := gallery_client.count_art_objects(filter)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return 0
	requests[request_id] = request
	await request.responded
	return request.response


func layout(filter: String, dense: bool) -> void:
	var request := EmptyRequest.new()
	var request_id := gallery_client.layout("res://Levels/moma-gallery.walls.json", filter, dense)
	if request_id == NULL_REQUEST_ID:
		push_error("Creating new layout failed!")
		# Oof, something went wrong.
		return
	requests[request_id] = request
	await request.responded
	print("Layout complete.")


func get_art_objects_for_gallery_wall(gallery_id: int, wall_id: String) -> Array[ArtObject]:
	var request := ArtObjectsRequest.new()
	var request_id := gallery_client.get_art_objects_for_gallery_wall(gallery_id, wall_id)
	if request_id == NULL_REQUEST_ID:
		# Oof, something went wrong.
		return []
	requests[request_id] = request
	await request.responded
	return request.response


func get_art_object_url(id: int) -> String:
	return gallery_client.get_art_object_url(id)


var fatal_error_message: String

var gallery_client: GalleryClient


func crash(message: String):
	OS.alert(message)
	get_tree().quit(1)


func copy_initial_db(db_filename: String) -> void:
	var GALLERY_DB_PATH := ROOT_DIR + db_filename

	if not FileAccess.file_exists(GALLERY_DB_PATH):
		const INITIAL_DB_PATH = "res://initial-db.sqlite"
		print("Copying initial DB to ", GALLERY_DB_PATH, ".")
		# I'd love to use DirAccess.copy_absolute() here because it
		# probably streams things, but it can't open the file and
		# basically seems to be completely broken:
		#
		#   https://github.com/godotengine/godot/issues/74105
		#
		# For now we'll just copy it in chunks manually.
		#
		# Beyond that, I don't really understand why I can't just
		# put this file alongside all the other files in the exported
		# project, rather than having to stuff it in the PCK/ZIP and
		# then extract it, but that doesn't seem to be something Godot
		# easily supports.
		var in_file := FileAccess.open(INITIAL_DB_PATH, FileAccess.READ)
		if not in_file:
			crash("Could not open initial DB for reading!")
			return
		var out_file := FileAccess.open(GALLERY_DB_PATH, FileAccess.WRITE)
		if not out_file:
			crash("Unable to write initial DB!")
		var total := 0
		const CHUNK_SIZE := 1000000
		while true:
			var data := in_file.get_buffer(CHUNK_SIZE)
			total += data.size()
			if data.size() == 0:
				break
			out_file.store_buffer(data)
		out_file.close()
		print("Wrote initial DB (", total, " bytes total).")


var ROOT_DIR: String

func _ready() -> void:
	if OS.has_feature("editor"):
		# Running from an editor binary.
		#
		# Store everything in a place that's convenient to access while developing,
		# relative to the project's root directory.
		#
		# If we change this dir, we will want to change where the CLI accesses things too.
		ROOT_DIR = "res://rust/cache/"
	else:
		# Running from an exported project.
		#
		# Store everything in the persistent user data directory:
		#
		#   https://docs.godotengine.org/en/stable/tutorials/io/data_paths.html#accessing-persistent-user-data-user
		ROOT_DIR = "user://"
	gallery_client = GalleryClient.new()
	copy_initial_db(gallery_client.default_db_filename())
	gallery_client.name = "GalleryClient"
	add_child(gallery_client)
	gallery_client.connect(ROOT_DIR)


func _process(_delta) -> void:
	if fatal_error_message:
		return
	fatal_error_message = gallery_client.take_fatal_error()
	if fatal_error_message:
		UserInterface.show_fatal_error(fatal_error_message)
		# TODO: It would be nice to let all requests know that an error occurred.
		requests.clear()
		return
	# TODO: If we're headless, possibly no need to handle max requests per frame.
	var start_time := Time.get_ticks_usec()
	while true:
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
		elif request is ArtObjectsRequest:
			var r: ArtObjectsRequest = request
			r.response = obj.take_art_objects()
			r.responded.emit()
		elif request is EmptyRequest:
			var r: EmptyRequest = request
			assert(obj.take_variant() == null)
			r.responded.emit()
		elif request is IntRequest:
			var r: IntRequest = request
			var result = obj.take_variant()
			assert(result is int)
			r.response = result
			r.responded.emit()
		else:
			assert(false, "Unknown request type, cannot fill response")
		var time_elapsed := Time.get_ticks_usec() - start_time
		if time_elapsed > MAX_USEC_PER_FRAME:
			if time_elapsed > WARNING_USEC_PER_FRAME:
				print("Warning: spent ", time_elapsed, " usec processing responses from Rust.")
			return