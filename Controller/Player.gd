extends CharacterBody3D
class_name Player

## How fast the player moves on the ground.
@export var base_speed := 6.0
## How high the player can jump in meters.
@export var jump_height := 1.2
## How fast the player falls after reaching max jump height.
@export var fall_multiplier := 2.5

## Whether this is the main player, or a remote player (or npc, etc)
@export var is_main_player: bool

## Whether this is player used in offline mode
@export var is_offline_mode_player: bool

## The peer ID of this player (multiplayer only, set at runtime).
@export var peer_id: int:
	set(value):
		peer_id = value
		$PlayerInput.set_multiplayer_authority(peer_id)

@export_category("Camera")
## How much moving the mouse moves the camera. Overwritten in settings.
@export var mouse_sensitivity: float = 0.00075
## Limits how low the player can look down.
@export var bottom_clamp: float = -90.0
## Limits how high the player can look up.
@export var top_clamp: float = 90.0

@export_category("Third Person")
## Limits how far the player can zoom in.
@export var min_zoom: float = 1.5
## Limits how far the player can zoom out.
@export var max_zoom: float = 6.0
## How quickly to zoom the camera
@export var zoom_sensitivity: float = 0.4

@onready var player_input: PlayerInput = $PlayerInput

# Get the gravity from the project settings to be synced with RigidBody nodes.
var gravity: float = ProjectSettings.get_setting("physics/3d/default_gravity")
# Stores the direction the player is trying to look this frame.
var _look := Vector2.ZERO

enum VIEW {
	FIRST_PERSON,
	THIRD_PERSON_BACK
}

# Updates the cameras to swap between first and third person.
var view := VIEW.FIRST_PERSON:
	set(value):
		view = value
		if not is_main_player:
			return
		match view:
			VIEW.FIRST_PERSON:
				# Get the fov of the current camera and apply it to the target.
				camera.fov = get_viewport().get_camera_3d().fov
				camera.current = true
				UserInterface.hide_reticle(false)
			VIEW.THIRD_PERSON_BACK:
				# Get the fov of the current camera and apply it to the target.
				third_person_camera.fov = get_viewport().get_camera_3d().fov
				third_person_camera.current = true
				UserInterface.hide_reticle(true)

# Control the target length of the third person camera arm..
var zoom := min_zoom:
	set(value):
		zoom = clamp(value, min_zoom, max_zoom)
		if value < min_zoom:
			# When the player zooms all the way in swap to first person.
			view = VIEW.FIRST_PERSON
		elif value > min_zoom:
			# When the player zooms out at all swap to third person.
			view = VIEW.THIRD_PERSON_BACK

@onready var camera: Camera3D = $SmoothCamera
@onready var third_person_camera: Camera3D = %ThirdPersonCamera
@onready var spring_arm_3d: SpringArm3D = %SpringArm3D

@onready var camera_target: Node3D = $CameraTarget
@onready var camera_origin = camera_target.position

@onready var animation_tree: AnimationTree = $AnimationTree
@onready var run_particles: GPUParticles3D = $BasePivot/RunParticles
@onready var jump_particles: GPUParticles3D = $BasePivot/JumpParticles

@onready var jump_audio: AudioStreamPlayer3D = %JumpAudio
@onready var run_audio: AudioStreamPlayer3D = %RunAudio

@onready var raycast: RayCast3D = $SmoothCamera/RayCast3D

var moving_painting: Moma.MovingPainting = null

func _ready() -> void:
	if is_offline_mode_player and not Lobby.IS_OFFLINE_MODE:
		# We're about to be removed from the scene tree, just exit.
		return
	elif not Lobby.IS_OFFLINE_MODE:
		if peer_id == multiplayer.get_unique_id():
			print("Main player ", peer_id, " spawned.")
			# This is the main player!
			is_main_player = true

	if is_main_player:
		camera.make_current()
		Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
		# Whenever the player loads in, give the autoload ui a reference to itself.
		UserInterface.update_player(self)
	else:
		# This is extremely annoying: just spawning a new player will make their
		# camera the current one, and I can't make it _not_ current in the scene
		# editor, so I guess I'll have to forcibly iterate through every single
		# player to find the main one and make it the main camera again.
		#
		# I guess I might have to refactor this thing to have the camera be a
		# completely separate element that's only attached to a player when
		# it's the main one.
		for p in get_tree().get_nodes_in_group("Player"):
			var player: Player = p
			if player.is_main_player:
				player.camera.make_current()


func _physics_process(delta: float) -> void:
	if is_main_player:
		frame_camera_rotation()
		smooth_camera_zoom(delta)

	# Add gravity.
	if not is_on_floor():
		# if holding jump and ascending be floaty.
		if velocity.y >= 0 and Input.is_action_pressed("ui_accept"):
			velocity.y -= gravity * delta
		else:
			# Double fall speed, after peak of jump or release of jump button.
			velocity.y -= gravity * delta * fall_multiplier
		
	# Handle jump.
	if is_main_player and Input.is_action_just_pressed("jump") and is_on_floor():
		# Projectile motion to turn jump height into a velocity.
		velocity.y = sqrt(jump_height * 2.0 * gravity)
		jump_particles.restart()
		#jump_audio.play()
		#run_audio.play()
	
	# Handle movement.
	var direction = get_movement_direction()
	if direction:
		velocity.x = lerp(velocity.x, direction.x * base_speed, base_speed * delta)
		velocity.z =  lerp(velocity.z, direction.z * base_speed, base_speed * delta)
	else:
		velocity.x = move_toward(velocity.x, 0, base_speed * delta * 5.0)
		velocity.z = move_toward(velocity.z, 0, base_speed * delta * 5.0)
	
	# Emit run particles when moving on the floor.
	run_particles.emitting = not direction.is_zero_approx() and is_on_floor()
		
	update_animation_tree()
	move_and_slide()

	if moving_painting:
		moving_painting.move_along_wall(raycast)

	if is_main_player and UserInterface.reticle.visible:
		var painting := Moma.try_to_find_painting_from_collision(raycast.get_collider())
		UserInterface.reticle.is_highlighted = painting != null

# Turn movent inputs into a locally oriented vector.
func get_movement_direction() -> Vector3:
	var input_dir := player_input.input_direction
	return (player_input.basis * Vector3(input_dir.x, 0, input_dir.y)).normalized()

# Apply the _look variables rotation to the camera.
func frame_camera_rotation() -> void:
	rotate_y(_look.x)
	player_input.basis = transform.basis
	# The smooth camera orients the camera to align with the target smoothly.
	camera_target.rotate_x(_look.y)
	camera_target.rotation.x = clamp(camera_target.rotation.x, 
		deg_to_rad(bottom_clamp), 
		deg_to_rad(top_clamp)
	)
	# Reset the _look variable so the same offset can't be reapplied.
	_look = Vector2.ZERO


# Blend the walking animation based on movement direction.
func update_animation_tree() -> void:
	# Get the local movement direction.
	var movement_direction := basis.inverse() * velocity / base_speed
	# Convert the direction to a Vector2 to select the correct movement animation.
	var animation_target = Vector2(movement_direction.x, -movement_direction.z)
	animation_tree.set("parameters/blend_position", animation_target)

func _unhandled_input(event: InputEvent) -> void:
	if not is_main_player:
		return
	# Update the _look variable to the latest mouse offset.
	if event is InputEventMouseMotion:
		var motion_event: InputEventMouseMotion = event
		if Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:
			_look = -motion_event.relative * mouse_sensitivity
	# Capture the mouse if it is uncaptured.
	if event.is_action_pressed("click"):
		if Input.get_mouse_mode() != Input.MOUSE_MODE_CAPTURED:
			Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
		elif moving_painting:
			moving_painting.finish_moving()
			moving_painting = null
			pass
		elif UserInterface.reticle.visible:
			moving_painting = Moma.MovingPainting.try_to_start_moving(raycast)

	if event.is_action_pressed("right_click") and UserInterface.reticle.visible:
		var painting := Moma.try_to_find_painting_from_collision(raycast.get_collider())
		if painting:
			painting.try_to_open_in_browser()

	# Camera controls.
	if event.is_action_pressed("toggle_view"):
		cycle_view()
	if event.is_action_pressed("zoom_in"):
		zoom -= zoom_sensitivity
	elif event.is_action_pressed("zoom_out"):
		zoom += zoom_sensitivity

	if event.is_action_pressed("toggle_wall_labels"):
		# This is set in project settings but I'm not sure how to keep it in sync with it.
		# If we change it here, we should change it there too.
		const WALL_LABELS_LAYER := 20
		var is_enabled := camera.get_cull_mask_value(WALL_LABELS_LAYER)
		camera.set_cull_mask_value(WALL_LABELS_LAYER, not is_enabled)


func cycle_view() -> void:
	# Swap from third to first person and vice versa.
	match view:
		VIEW.FIRST_PERSON:
			view = VIEW.THIRD_PERSON_BACK
			# Set the default third person zoom to halfway between min and max.
			zoom = lerp(min_zoom, max_zoom, 0.5)
		VIEW.THIRD_PERSON_BACK:
			view = VIEW.FIRST_PERSON
		_:
			view = VIEW.FIRST_PERSON

# Interpolate the third person distance to the target length.
func smooth_camera_zoom(delta: float) -> void:
	spring_arm_3d.spring_length = lerp(
		spring_arm_3d.spring_length,
		zoom,
		delta * 10.0
	)

# Play a footstep sound effect when moving.
func _on_footstep_timer_timeout() -> void:
	if is_on_floor() and get_movement_direction():
		#run_audio.play()
		pass
