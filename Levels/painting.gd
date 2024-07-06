class_name Painting

extends Node3D


const PAINTING_SURFACE_IDX = 1

## The minimum ratio of the painting's size on screen to its
## image texture's size that will trigger loading the large
## version of the image.
const LARGE_IMAGE_AREA_RATIO_THRESHOLD = 1.75

const WALL_LABEL_SECONDARY_TOP_PADDING = 0.005

const WALL_LABEL_TERTIARY_TOP_PADDING = 0.02

var painting_surface_material: StandardMaterial3D

var original_albedo_color: Color

var loaded_large_image := false

@onready var painting: MeshInstance3D = %painting/Painting

@onready var collision_shape: CollisionShape3D = $painting/Painting/StaticBody3D/CollisionShape3D

@onready var wall_label_primary: Label3D = %wall_label_primary

@onready var wall_label_secondary: Label3D = %wall_label_secondary

@onready var wall_label_tertiary: Label3D = %wall_label_tertiary

## The scaling applied to the actual painting canvas, set by the server in multiplayer. (We can't
## simply synchronize the actual scale because it's in an imported scene that our synchronizer
## seems to be unable to access.)
@export var inner_painting_scale: Vector3

## The met object ID of the painting, set by the server.
@export var met_object_id: int

## The title of the painting, set by the server.
@export var title: String

## The artist of the painting, set by the server.
@export var artist: String

## The medium of the painting (e.g., "oil on canvas"), set by the server.
@export var medium: String

## The date the painting was created, set by the server.
@export var date: String

var small_image_texture: ImageTexture

func _ready():
	if inner_painting_scale:
		configure_wall_label()
		painting.set_scale(inner_painting_scale)
	else:
		print("Warning: No inner_painting_scale available for painting!")
	if met_object_id:
		# TODO: Only do this when the player is near the painting.
		var small_image := await MetObjects.fetch_small_image(met_object_id)
		if not is_inside_tree():
			# We despawned, exit.
			return
		if not small_image:
			# Oof, fetching the image failed.
			visible = false
			return
		small_image.generate_mipmaps()
		small_image_texture = ImageTexture.create_from_image(small_image)
		var material: StandardMaterial3D = painting.mesh.surface_get_material(PAINTING_SURFACE_IDX)
		painting_surface_material = material.duplicate()
		painting_surface_material.albedo_color = Color.TRANSPARENT
		painting_surface_material.albedo_texture = small_image_texture

		# It's easier to experiment with these settings via script rather than setting them in Blender
		# and constantly re-exporting/re-importing.
		painting_surface_material.specular_mode = BaseMaterial3D.SPECULAR_DISABLED
		painting_surface_material.metallic = 0.0
		painting_surface_material.roughness = 1.0
		if not UserInterface.potato_mode:
			painting_surface_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_LINEAR_WITH_MIPMAPS_ANISOTROPIC

		painting.set_surface_override_material(PAINTING_SURFACE_IDX, painting_surface_material)
	else:
		print("Warning: No met_object_id available for painting!")


func _get_side_multiplier(value: float) -> float:
	if value == 0.0:
		return 0.0
	elif value < 0.0:
		return -1.0
	return 1.0


func configure_wall_label() -> void:
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
	await get_tree().process_frame
	if not is_inside_tree():
		return

	wall_label_secondary.position.x = left_edge
	wall_label_secondary.position.y = wall_label_primary.position.y - wall_label_primary.get_aabb().size.y - WALL_LABEL_SECONDARY_TOP_PADDING
	var date_suffix := ""
	if date:
		date_suffix = ", " + date
	wall_label_secondary.text = _default_str(title, "Untitled") + date_suffix
	wall_label_secondary.visible = true

	await get_tree().process_frame
	if not is_inside_tree():
		return
	wall_label_tertiary.position.x = left_edge
	wall_label_tertiary.position.y = wall_label_secondary.position.y - wall_label_secondary.get_aabb().size.y - WALL_LABEL_TERTIARY_TOP_PADDING
	wall_label_tertiary.text = medium
	wall_label_tertiary.visible = true


func _default_str(value: String, default: String) -> String:
	if value:
		return value
	return default


func init_with_met_object(object: MetObject):
	inner_painting_scale = Vector3(object.width, object.height, 1.0)
	met_object_id = object.object_id
	artist = object.artist
	title = object.title
	medium = object.medium
	date = object.date


func try_to_open_in_browser():
	OS.shell_open("https://www.metmuseum.org/art/collection/search/" + str(met_object_id))


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
			print("Evicting large image for met object id ", old_painting.met_object_id, ".")
			old_painting.painting_surface_material.albedo_texture = old_painting.small_image_texture
			old_painting.loaded_large_image = false
	paintings_with_large_images.push_back(weakref(self))
	large_image.generate_mipmaps()
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

	if loaded_large_image:
		# We already loaded the large image, nothing to do.
		return

	var unproj_size := _get_approx_unprojected_rect(camera).size
	var small_image_size := small_image_texture.get_size()
	var area_ratio := (unproj_size.x * unproj_size.y) / (small_image_size.x * small_image_size.y)

	if area_ratio > LARGE_IMAGE_AREA_RATIO_THRESHOLD:
		loaded_large_image = true
		var large_image := await MetObjects.fetch_large_image(met_object_id)
		if not is_inside_tree():
			# We despawned, exit.
			return
		if not large_image:
			# Downloading failed.
			return
		_set_large_image(large_image)
		var new_size := large_image.get_size()
		print("Loaded large ", new_size.x, "x", new_size.y, " image for met object id ", met_object_id, " (area ratio was ", area_ratio, ").")
