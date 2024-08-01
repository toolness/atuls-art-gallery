extends Node

const NULL_REQUEST_ID = 0

## Try to only spend these many microseconds per frame processing
## responses, as we don't want to skip frames.
const MAX_USEC_PER_FRAME = 8000

## If we spend more than these many microseconds processing
## responses, log a warning.
const WARNING_USEC_PER_FRAME = 16000

var requests = {}

class ArtObjectsRequest:
	var response: Array[ArtObject]
	signal responded

class ImageRequest:
	var image_path: String
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
		crash("Invalid image size: " + size)
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
	var GALLERY_DB_PATH := PersistedConfig.ROOT_DIR + db_filename

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

func _ready() -> void:
	gallery_client = GalleryClient.new()
	copy_initial_db(gallery_client.default_db_filename())
	gallery_client.name = "GalleryClient"
	add_child(gallery_client)
	gallery_client.connect(PersistedConfig.ROOT_DIR)

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
	var tracker := ElapsedTimeTracker.new()
	while true:
		var obj := gallery_client.poll()
		_handle_gallery_response(obj)

		if tracker.has_too_much_time_elapsed():
			return

		var image_request := image_loading_thread.get_loaded_image()
		if image_request:
			image_request.responded.emit()

		if tracker.has_too_much_time_elapsed():
			return

class ElapsedTimeTracker:
	var start_time := Time.get_ticks_usec()

	func has_too_much_time_elapsed() -> bool:
		var time_elapsed := Time.get_ticks_usec() - start_time
		if time_elapsed > MAX_USEC_PER_FRAME:
			if time_elapsed > WARNING_USEC_PER_FRAME:
				print("Warning: spent ", time_elapsed, " usec processing responses.")
			return true
		return false

func _handle_gallery_response(obj: GalleryResponse):
	if not obj:
		return
	if not requests.has(obj.request_id):
		print("Warning: request #", obj.request_id, " does not exist.")
		return
	var request = requests[obj.request_id]
	requests.erase(obj.request_id)
	if request is ImageRequest:
		var r: ImageRequest = request
		var path = obj.take_variant()
		if path is String:
			r.image_path = path
			image_loading_thread.load_image(r)
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

# Ideally we'd do all this from Rust, but support for multi-threading in gdext
# is still evolving, so we're doing it here.
class ImageLoadingThread:
	var thread: Thread
	var semaphore: Semaphore
	var mutex: Mutex

	# Access to these must all be protected by Mutxes.
	var _images_to_load: Array[ImageRequest]
	var _loaded_images: Array[ImageRequest]
	var _should_exit: bool

	func start():
		print("Spawning image loading thread.")
		mutex = Mutex.new()
		semaphore = Semaphore.new()
		_images_to_load = []
		_loaded_images = []
		_should_exit = false
		thread = Thread.new()
		thread.start(_run)

	func load_image(request: ImageRequest):
		assert(request.image_path is String and len(request.image_path) > 0)
		if not thread:
			self.start()
		mutex.lock()
		_images_to_load.push_back(request)
		mutex.unlock()
		semaphore.post()

	func get_loaded_image() -> ImageRequest:
		if not thread:
			return null
		mutex.lock()
		var request: ImageRequest = _loaded_images.pop_back()
		mutex.unlock()
		return request

	func join():
		if thread:
			print("Joining image loading thread.")
			mutex.lock()
			_should_exit = true
			mutex.unlock()
			semaphore.post()
			thread.wait_to_finish()

	func _run():
		while true:
			semaphore.wait()
			mutex.lock()
			var should_exit := _should_exit
			var image_to_load: ImageRequest = _images_to_load.pop_back()
			mutex.unlock()
			if should_exit:
				return
			if image_to_load:
				var image := Image.load_from_file(image_to_load.image_path)
				if image:
					image.generate_mipmaps()
				# It's possible that we could convert to an ImageTexture here in this other thread,
				# but it's unclear if that would actually improve performance. From the Godot
				# documentation [1]:
				#
				# > You should avoid calling functions involving direct interaction with the GPU
				# > on other threads, such as creating new textures or modifying and
				# > retrieving image data, these operations can lead to performance stalls
				# > because they require synchronization with the RenderingServer, as data
				# > needs to be transmitted to or updated on the GPU.
				#
				# [1]: https://docs.godotengine.org/en/stable/tutorials/performance/thread_safe_apis.html#rendering
				image_to_load.response = image
				mutex.lock()
				_loaded_images.push_back(image_to_load)
				mutex.unlock()

var image_loading_thread := ImageLoadingThread.new()

func _exit_tree():
	image_loading_thread.join()
