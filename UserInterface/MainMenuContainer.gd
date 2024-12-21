extends CenterContainer
class_name MainMenuContainer

@onready var start_button: Button = %StartButton

@onready var join_button: Button = %JoinButton

@onready var host_button: Button = %HostButton

@onready var settings_button: Button = %SettingsButton

@onready var quit_button: Button = %QuitButton

@onready var title_label: Label = %TitleLabel

func focus():
	start_button.grab_focus()


# Called when the node enters the scene tree for the first time.
func _ready():
	_on_gallery_name_changed()
	UserInterface.gallery_name_changed.connect(_on_gallery_name_changed)


func _on_gallery_name_changed():
	title_label.text = UserInterface.gallery_name
