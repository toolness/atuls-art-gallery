extends WorldEnvironment

@onready var potato_mode_environment: Environment = preload("res://Levels/environment_potato.tres")

@onready var default_environment: Environment = preload("res://Levels/environment.tres")


# Called when the node enters the scene tree for the first time.
func _ready():
	_on_potato_mode_changed()
	UserInterface.potato_mode_changed.connect(_on_potato_mode_changed)
	_on_global_illumination_changed()
	UserInterface.global_illumination_changed.connect(_on_global_illumination_changed)


func _on_potato_mode_changed():
	if UserInterface.potato_mode:
		environment = potato_mode_environment
	else:
		environment = default_environment


func _on_global_illumination_changed():
	default_environment.sdfgi_enabled = UserInterface.global_illumination
