extends WorldEnvironment

@onready var potato_mode_environment: Environment = preload("res://Levels/environment_potato.tres")

@onready var default_environment: Environment = preload("res://Levels/environment.tres")


# Called when the node enters the scene tree for the first time.
func _ready():
	_on_potato_mode_changed(UserInterface.potato_mode)
	UserInterface.potato_mode_changed.connect(_on_potato_mode_changed)


func _on_potato_mode_changed(potato_mode: bool):
	if potato_mode:
		environment = potato_mode_environment
	else:
		environment = default_environment
