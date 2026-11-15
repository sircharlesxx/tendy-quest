extends Node

signal wave_started(wave_number: int, total_waves: int)

@export var enemy_scene: PackedScene
@export var extra_enemy_scenes: Array[PackedScene] = []
@export var spawn_points_path: NodePath
@export var wave_interval: float = 18.0
@export var max_active_enemies: int = 12
@export var wave_counts: Array[int] = [3, 4, 5]

var current_wave: int = 0
var spawn_points: Array[Marker3D] = []
var wave_timer: Timer

func _ready() -> void:
	var sp_container := get_node_or_null(spawn_points_path)
	if sp_container:
		for child in sp_container.get_children():
			if child is Marker3D:
				spawn_points.append(child)

	wave_timer = Timer.new()
	wave_timer.wait_time = wave_interval
	wave_timer.one_shot = false
	wave_timer.timeout.connect(_spawn_next_wave)
	add_child(wave_timer)

	_spawn_next_wave.call_deferred()
	wave_timer.start()

func _spawn_next_wave() -> void:
	if current_wave >= wave_counts.size():
		wave_timer.stop()
		return
	if enemy_scene == null or spawn_points.is_empty():
		return

	current_wave += 1
	var desired_count := wave_counts[current_wave - 1]
	var active_count := get_tree().get_nodes_in_group("enemies").size()
	var to_spawn := mini(desired_count, maxi(0, max_active_enemies - active_count))

	var pool: Array[PackedScene] = []
	if enemy_scene:
		pool.append(enemy_scene)
	for s in extra_enemy_scenes:
		if s:
			pool.append(s)
	if pool.is_empty():
		return

	for i in to_spawn:
		var marker := spawn_points[(current_wave * 3 + i) % spawn_points.size()]
		var chosen := pool[(current_wave + i) % pool.size()]
		var enemy := chosen.instantiate() as Node3D
		enemy.position = marker.global_position
		get_parent().add_child(enemy)

	wave_started.emit(current_wave, wave_counts.size())
	print("Wave %d/%d started! Spawned %d veggie monsters." % [current_wave, wave_counts.size(), to_spawn])
