class_name Painting

extends Node3D


const PAINTING_SURFACE_IDX = 1

## The minimum ratio of the painting's size on screen to its
## image texture's size that will trigger loading the large
## version of the image.
const LARGE_IMAGE_AREA_RATIO_THRESHOLD = 1.75

const WALL_LABEL_SECONDARY_TOP_PADDING = 0.005

const WALL_LABEL_TERTIARY_TOP_PADDING = 0.02

## The maximum distance, in meters, from the player a painting
## must be in order to start loading its small image.
##
## If the player is further than this distance away, the small
## image won't load, as it's assumed that the player can't see
## the painting.
const SMALL_IMAGE_DISTANCE_THRESHOLD := 30.0

var painting_surface_material: StandardMaterial3D

var original_albedo_color: Color

var started_loading_small_image := false

var started_loading_large_image := false

var is_duplicate := false

@onready var painting: MeshInstance3D = %painting/Painting

@onready var collision_shape: CollisionShape3D = $painting/Painting/StaticBody3D/CollisionShape3D

@onready var wall_label_primary: Label3D = %wall_label_primary

@onready var wall_label_secondary: Label3D = %wall_label_secondary

@onready var wall_label_tertiary: Label3D = %wall_label_tertiary

## The scaling applied to the actual painting canvas, set by the server in multiplayer. (We can't
## simply synchronize the actual scale because it's in an imported scene that our synchronizer
## seems to be unable to access.)
@export var inner_painting_scale: Vector3

## The art object ID of the painting, set by the server.
@export var art_object_id: int

## The title of the painting, set by the server.
@export var title: String

## The artist of the painting, set by the server.
@export var artist: String

## The medium of the painting (e.g., "oil on canvas"), set by the server.
@export var medium: String

## The date the painting was created, set by the server.
@export var date: String

## The collection the painting is part of, set by the server.
@export var collection: String

var small_image_texture: ImageTexture

func _get_initial_albedo_color() -> Color:
	# We try to set this to be the color of the wall behind the painting, so
	# it blends with the wall while it's still loading.

	# TODO: This coupling to the parent isn't great, ideally it should be
	# passed down to us from the parent. However, that won't work well in
	# multiplayer scenarios because we're being spawned via replication
	# and we don't want to have to add yet another variable to sync as
	# it will increase network bandwidth, so we'll just use this hack
	# for now.
	var parent := get_parent()
	if parent is Moma:
		var moma_parent: Moma = parent
		return moma_parent.get_gallery_wall_color()

	return Color.TRANSPARENT

func _ready():
	if inner_painting_scale:
		configure_wall_label()
		painting.set_scale(inner_painting_scale)
	else:
		print("Warning: No inner_painting_scale available for painting!")
	if not art_object_id:
		print("Warning: No art_object_id available for painting!")
	var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
	painting_surface_material = material.duplicate()
	painting_surface_material.albedo_color = _get_initial_albedo_color()

	# It's easier to experiment with these settings via script rather than setting them in Blender
	# and constantly re-exporting/re-importing.
	painting_surface_material.specular_mode = BaseMaterial3D.SPECULAR_DISABLED
	painting_surface_material.metallic = 0.0
	painting_surface_material.roughness = 1.0
	if not UserInterface.potato_mode:
		painting_surface_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_LINEAR_WITH_MIPMAPS_ANISOTROPIC

	painting.set_surface_override_material(PAINTING_SURFACE_IDX, painting_surface_material)

	# Note that we don't want to see if the player is close enough to load the small
	# image yet, because our painting may not yet have been moved to its final position
	# when it first enters the scene tree.


## This should only ever be used to fix a bug with lighting whereby moving a painting
## to somewhere with different illumination preserves the illumination of its original
## location, making it look horribly over/under-exposed.
static func respawn_to_fix_stupid_lighting_bugs(old_painting: Painting):
	var parent = old_painting.get_parent()
	if not parent:
		push_error("Unable to respawn painting, it has no parent.")
		return
	var new_painting: Painting = old_painting.duplicate()
	new_painting.is_duplicate = true
	parent.remove_child(old_painting)
	old_painting.free()
	parent.add_child(new_painting)
	print(
		"Respawned painting with name=", new_painting.name, " object_id=", new_painting.art_object_id,
		" to work around stupid lighting bugs."
	)


func _maybe_load_small_image():
	if started_loading_small_image:
		return
	if not art_object_id:
		return
	var player: Player = UserInterface.main_player
	if not player:
		return
	var distance_from_player := player.global_position.distance_to(global_position)
	if distance_from_player > SMALL_IMAGE_DISTANCE_THRESHOLD:
		return
	# print("Loading painting with art object id ", art_object_id, " (", distance_from_player, " m from player).")
	started_loading_small_image = true
	var small_image := await ArtObjects.fetch_small_image(art_object_id)
	if not is_inside_tree():
		# We despawned, exit.
		return
	if not small_image:
		# Oof, fetching the image failed.
		visible = false
		return
	small_image_texture = ImageTexture.create_from_image(small_image)
	painting_surface_material.albedo_texture = small_image_texture


func _process(_delta: float):
	_maybe_load_small_image()
	if started_loading_small_image:
		set_process(false)


func _get_side_multiplier(value: float) -> float:
	if value == 0.0:
		return 0.0
	elif value < 0.0:
		return -1.0
	return 1.0


func _wait_for_bounding_box_recomputes():
	var num_frames := 1
	if Lobby.IS_CLIENT:
		# Absolutely no idea why this takes longer when we're connected to a server,
		# but if we don't do this, the bounding boxes don't seem to be accurate.
		num_frames = 5
	for i in range(num_frames):
		await get_tree().process_frame
		if not is_inside_tree():
			return


func configure_wall_label() -> void:
	if is_duplicate:
		# This is a duplicate, the wall label has already been configured.
		return
	var aabb_size := painting.get_aabb().size
	var x := wall_label_primary.position.x
	var wall_label_x_offset := absf(x) - aabb_size.x / 2
	var left_edge := _get_side_multiplier(x) * (inner_painting_scale.x / 2 + wall_label_x_offset)
	wall_label_primary.position.x = left_edge
	var y := wall_label_primary.position.y
	var wall_label_y_offset := absf(y) - aabb_size.y / 2
	wall_label_primary.position.y = _get_side_multiplier(y) * (inner_painting_scale.y / 2 + wall_label_y_offset)
	wall_label_primary.text = _default_str(artist, "Anonymous")

	# This is a bit annoying, we have to wait a full frame for the primary label to populate its AABB.
	# I was hoping that generate_triangle_mesh() would populate it, but it doesn't. So we'll have to hide the
	# wall labels that go below the first one, and incrementally position them each a frame apart from one another,
	# displaying them only when we know their position.
	wall_label_secondary.visible = false
	wall_label_tertiary.visible = false
	await _wait_for_bounding_box_recomputes()
	if not is_inside_tree():
		return

	wall_label_secondary.position.x = left_edge
	wall_label_secondary.position.y = wall_label_primary.position.y - wall_label_primary.get_aabb().size.y - WALL_LABEL_SECONDARY_TOP_PADDING
	var date_suffix := ""
	if date:
		date_suffix = ", " + date
	wall_label_secondary.text = _default_str(title, "Untitled") + date_suffix
	wall_label_secondary.visible = true

	await _wait_for_bounding_box_recomputes()
	if not is_inside_tree():
		return
	wall_label_tertiary.position.x = left_edge
	wall_label_tertiary.position.y = wall_label_secondary.position.y - wall_label_secondary.get_aabb().size.y - WALL_LABEL_TERTIARY_TOP_PADDING
	wall_label_tertiary.text = medium + "\n" + collection
	wall_label_tertiary.visible = true


func _default_str(value: String, default: String) -> String:
	if value:
		return value
	return default


func init_with_art_object(object: ArtObject):
	inner_painting_scale = Vector3(object.width, object.height, 1.0)
	art_object_id = object.object_id
	artist = object.artist
	title = object.title
	medium = object.medium
	date = object.date
	collection = object.collection


func try_to_open_in_browser():
	OS.shell_open(ArtObjects.get_art_object_url(art_object_id))


func start_interactive_placement():
	original_albedo_color = painting_surface_material.albedo_color
	painting_surface_material.albedo_color = Color.GREEN
	collision_shape.disabled = true


func finish_interactive_placement():
	painting_surface_material.albedo_color = original_albedo_color
	collision_shape.disabled = false


## Return the _approximate_ rectange corresponding to the image's
## unproject size in the given camera's viewport. This is approximate
## because it doesn't account for skew, e.g. it just assumes the player
## is looking at the painting head-on.  It is also the rect of the
## _back_ of the painting rather than the front.
func _get_approx_unprojected_rect(camera: Camera3D) -> Rect2:
	var top_left := painting.global_position + painting.global_transform.basis.y / 2 - painting.global_transform.basis.x / 2
	var bottom_right := painting.global_position - painting.global_transform.basis.y / 2 + painting.global_transform.basis.x / 2
	var top_left_unproj := camera.unproject_position(top_left)
	var bottom_right_unproj := camera.unproject_position(bottom_right)
	var width := bottom_right_unproj.x - top_left_unproj.x
	var height := bottom_right_unproj.y - top_left_unproj.y
	return Rect2(top_left_unproj.x, top_left_unproj.y, width, height)


## The most paintings with large images we'll have in-memory at once.
const MAX_PAINTINGS_WITH_LARGE_IMAGES = 5

## An array of weak references to paintings with large images. We'll keep
## them weak so they can get freed from memory without us having to
## explicitly free them ourselves.
static var paintings_with_large_images: Array[WeakRef] = []


## Sets the texture for the painting to the given large image.
## If too many other paintings have large images, the oldest one will switch
## back to a small image, to preserve memory.
func _set_large_image(large_image: Image):
	while len(paintings_with_large_images) >= MAX_PAINTINGS_WITH_LARGE_IMAGES:
		var weak_old_painting: WeakRef = paintings_with_large_images.pop_front()
		var old_painting: Painting = weak_old_painting.get_ref()
		if old_painting and old_painting.is_inside_tree():
			print("Evicting large image for art object id ", old_painting.art_object_id, ".")
			old_painting.painting_surface_material.albedo_texture = old_painting.small_image_texture
			old_painting.started_loading_large_image = false
	paintings_with_large_images.push_back(weakref(self))
	var large_image_texture := ImageTexture.create_from_image(large_image)
	# TODO: Godot actually supports the concept of a "detail texture, with the ability
	# to mix between standard and detail. We could potentially use this feature to tween
	# between the standard and the large version of the image:
	#
	#   https://docs.godotengine.org/en/stable/tutorials/3d/standard_material_3d.html#detail
	painting_surface_material.albedo_texture = large_image_texture


func handle_player_looking_at(camera: Camera3D):
	if UserInterface.potato_mode:
		# Don't load large images in potato mode.
		return

	if not small_image_texture:
		# We haven't loaded a small image yet.
		return

	if started_loading_large_image:
		# We already loaded (or started loading) the large image, nothing to do.
		return

	var unproj_size := _get_approx_unprojected_rect(camera).size
	var small_image_size := small_image_texture.get_size()
	var area_ratio := (unproj_size.x * unproj_size.y) / (small_image_size.x * small_image_size.y)

	if area_ratio > LARGE_IMAGE_AREA_RATIO_THRESHOLD:
		started_loading_large_image = true
		var large_image := await ArtObjects.fetch_large_image(art_object_id)
		if not is_inside_tree():
			# We despawned, exit.
			return
		if not large_image:
			# Downloading failed.
			return
		_set_large_image(large_image)
		var new_size := large_image.get_size()
		print("Loaded large ", new_size.x, "x", new_size.y, " image for art object id ", art_object_id, " (area ratio was ", area_ratio, ").")
