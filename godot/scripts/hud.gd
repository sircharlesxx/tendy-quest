extends CanvasLayer

@export var player_path: NodePath
@export var wave_manager_path: NodePath

@onready var tendy_label: Label = $Panel/VBoxContainer/TendyLabel
@onready var craving_label: Label = $Panel/VBoxContainer/CravingLabel
@onready var wave_label: Label = $Panel/VBoxContainer/WaveLabel

func _ready() -> void:
	var player := get_node_or_null(player_path)
	if player and player.has_signal("stats_changed"):
		player.stats_changed.connect(_on_player_stats_changed)

	var wave_manager := get_node_or_null(wave_manager_path)
	if wave_manager and wave_manager.has_signal("wave_started"):
		wave_manager.wave_started.connect(_on_wave_started)

func _on_player_stats_changed(tendies: int, craving: float) -> void:
	tendy_label.text = "Tendies: %d" % tendies
	craving_label.text = "Craving: %.0f%%" % craving

func _on_wave_started(wave_number: int, total_waves: int) -> void:
	wave_label.text = "Wave: %d / %d" % [wave_number, total_waves]
