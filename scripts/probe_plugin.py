"""Load the actual Windows DLL into a mock SCS host; no game or hardware required."""
import ctypes as c
from pathlib import Path
import struct
import sys
import time


def check_pe(path):
    image = path.read_bytes()
    pe = struct.unpack_from('<I', image, 0x3C)[0]
    assert image[pe:pe + 4] == b'PE\0\0'
    assert struct.unpack_from('<H', image, pe + 4)[0] == 0x8664, 'DLL must be x64'
    optional = pe + 24
    assert struct.unpack_from('<H', image, optional)[0] == 0x20B
    export_rva = struct.unpack_from('<I', image, optional + 112)[0]
    count = struct.unpack_from('<H', image, pe + 6)[0]
    table = optional + struct.unpack_from('<H', image, pe + 20)[0]

    def offset(rva):
        for index in range(count):
            section = table + index * 40
            virtual_size, address, raw_size, raw = struct.unpack_from('<IIII', image, section + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                return raw + rva - address
        raise AssertionError(f'RVA outside sections: {rva:x}')

    exports = offset(export_rva)
    name_count = struct.unpack_from('<I', image, exports + 24)[0]
    names = offset(struct.unpack_from('<I', image, exports + 32)[0])
    found = set()
    for index in range(name_count):
        start = offset(struct.unpack_from('<I', image, names + index * 4)[0])
        found.add(image[start:image.index(b'\0', start)].decode('ascii'))
    assert found == {'scs_input_init', 'scs_input_shutdown', 'scs_telemetry_init', 'scs_telemetry_shutdown'}, found


def probe(path):
    check_pe(path)
    api = c.WinDLL(str(path))
    Log = c.WINFUNCTYPE(None, c.c_int32, c.c_char_p)

    class Common(c.Structure):
        _fields_ = [('game_name', c.c_char_p), ('game_id', c.c_char_p), ('version', c.c_uint32), ('padding', c.c_uint32), ('log', Log)]

    class Input(c.Structure):
        _fields_ = [('name', c.c_char_p), ('display', c.c_char_p), ('kind', c.c_uint32), ('padding', c.c_uint32)]

    class Event(c.Structure):
        _fields_ = [('index', c.c_uint32), ('payload', c.c_uint32 * 6)]

    class Value(c.Structure):
        _fields_ = [('kind', c.c_uint32), ('padding', c.c_uint32), ('payload', c.c_uint64 * 5)]

    InputCallback = c.WINFUNCTYPE(c.c_int32, c.POINTER(Event), c.c_uint32, c.c_void_p)
    ActiveCallback = c.WINFUNCTYPE(None, c.c_uint8, c.c_void_p)
    EventCallback = c.WINFUNCTYPE(None, c.c_uint32, c.c_void_p, c.c_void_p)
    ChannelCallback = c.WINFUNCTYPE(None, c.c_char_p, c.c_uint32, c.POINTER(Value), c.c_void_p)

    class Device(c.Structure):
        _fields_ = [('name', c.c_char_p), ('display', c.c_char_p), ('kind', c.c_uint32), ('count', c.c_uint32), ('inputs', c.POINTER(Input)), ('context', c.c_void_p), ('active', ActiveCallback), ('event', InputCallback)]

    RegisterDevice = c.WINFUNCTYPE(c.c_int32, c.POINTER(Device))
    RegisterEvent = c.WINFUNCTYPE(c.c_int32, c.c_uint32, EventCallback, c.c_void_p)
    UnregisterEvent = c.WINFUNCTYPE(c.c_int32, c.c_uint32)
    RegisterChannel = c.WINFUNCTYPE(c.c_int32, c.c_char_p, c.c_uint32, c.c_uint32, c.c_uint32, ChannelCallback, c.c_void_p)
    UnregisterChannel = c.WINFUNCTYPE(c.c_int32, c.c_char_p, c.c_uint32, c.c_uint32)

    class InputParams(c.Structure):
        _fields_ = [('common', Common), ('register', RegisterDevice)]

    class TelemetryParams(c.Structure):
        _fields_ = [('common', Common), ('register_event', RegisterEvent), ('unregister_event', UnregisterEvent), ('register_channel', RegisterChannel), ('unregister_channel', UnregisterChannel)]

    for definition, size in [(Common, 32), (InputParams, 40), (TelemetryParams, 64), (Input, 24), (Device, 56), (Event, 28), (Value, 48)]:
        assert c.sizeof(definition) == size, definition

    callbacks = {}
    events = {}
    channels = {}
    failures = []
    fail_second_channel = False
    input_names = [b'lblinkerh', b'rblinkerh', b'lightoff', b'lightpark', b'lighton', b'wipers0', b'wipers1', b'wipers2', b'wipers3', b'lighthorn', b'hblight', b'horn', b'flasher4way', b'gear0', b'geardrive', b'gearreverse', b'parkingbrake', b'cruiectrl', b'cruiectrlres', b'cruiectrlinc', b'cruiectrldec']

    @Log
    def log(level, text):
        print(text.decode('utf-8'))

    @RegisterDevice
    def register_device(pointer):
        device = pointer.contents
        names = [device.inputs[i].name for i in range(device.count)]
        if device.kind != 2 or names != input_names:
            failures.append('incorrect semantic device registration')
            return -7
        # Copy callback addresses; the registration structure itself is temporary.
        callbacks['event'] = InputCallback(c.cast(device.event, c.c_void_p).value)
        callbacks['active'] = ActiveCallback(c.cast(device.active, c.c_void_p).value)
        return 0

    @RegisterEvent
    def register_event(event, callback, context):
        events[event] = (EventCallback(c.cast(callback, c.c_void_p).value), context)
        return 0

    @UnregisterEvent
    def unregister_event(event):
        events.pop(event, None)
        return 0

    number_kinds = {b'truck.cruise_control': 5, b'truck.navigation.speed.limit': 5, b'truck.speed': 5, b'truck.displayed.gear': 2}

    @RegisterChannel
    def register_channel(name, index, kind, flags, callback, context):
        if fail_second_channel and name == b'truck.rblinker':
            return -4
        if index != 0xFFFFFFFF or kind != number_kinds.get(name, 1) or flags != 3:
            failures.append('incorrect channel registration')
            return -7
        channels[name] = (ChannelCallback(c.cast(callback, c.c_void_p).value), context)
        return 0

    @UnregisterChannel
    def unregister_channel(name, index, kind):
        channels.pop(name, None)
        return 0

    common = Common(b'Mock ETS2', b'eut2', 0x10000, 0, log)
    inputs = InputParams(common, register_device)
    telemetry = TelemetryParams(common, register_event, unregister_event, register_channel, unregister_channel)
    api.scs_input_init.argtypes = [c.c_uint32, c.POINTER(InputParams)]
    api.scs_input_init.restype = c.c_int32
    api.scs_telemetry_init.argtypes = [c.c_uint32, c.POINTER(TelemetryParams)]
    api.scs_telemetry_init.restype = c.c_int32
    for name in ['scs_input_shutdown', 'scs_telemetry_shutdown']:
        getattr(api, name).argtypes = []
        getattr(api, name).restype = None

    assert api.scs_input_init(0x20000, None) == -1
    assert api.scs_input_init(0x10000, None) == -2
    assert api.scs_telemetry_init(0x20000, None) == -1
    for rejected in [None, b'', b'unknown', b'amtrucks']:
        inputs.common.game_id = rejected
        telemetry.common.game_id = rejected
        assert api.scs_input_init(0x10000, c.byref(inputs)) == -1
        assert api.scs_telemetry_init(0x10001, c.byref(telemetry)) == -1
        assert not callbacks and not events and not channels
    inputs.common.game_id = b'eut2'
    telemetry.common.game_id = b'eut2'
    fail_second_channel = True
    assert api.scs_telemetry_init(0x10001, c.byref(telemetry)) == -4
    assert not events and not channels, 'failed init must roll back registrations'
    fail_second_channel = False

    for cycle in range(6):
        game_id = b'eut2' if cycle % 2 == 0 else b'ats'
        inputs.common.game_id = game_id
        telemetry.common.game_id = game_id
        # Exercise both API initialization and shutdown orders.
        if cycle % 3 == 1:
            assert api.scs_input_init(0x10000, c.byref(inputs)) == 0
            assert api.scs_telemetry_init(0x10000, c.byref(telemetry)) == 0
        else:
            assert api.scs_telemetry_init(0x10001, c.byref(telemetry)) == 0
            assert api.scs_input_init(0x10000, c.byref(inputs)) == 0
        assert api.scs_input_init(0x10000, c.byref(inputs)) == -3
        callback, context = events[4]
        callback(4, None, context)
        value = Value(1, 0, (c.c_uint64 * 5)())
        for name, (callback, context) in channels.items():
            callback(name, 0xFFFFFFFF, c.byref(value), context)
        assert set(number_kinds) <= set(channels)
        assert len(channels) == 11
        for name, kind in number_kinds.items():
            callback, context = channels[name]
            number = Value(kind, 0, (c.c_uint64 * 5)())
            if kind == 5:
                c.cast(c.byref(number, 8), c.POINTER(c.c_float))[0] = 20.0
            else:
                c.cast(c.byref(number, 8), c.POINTER(c.c_int32))[0] = -1
            callback(name, 0xFFFFFFFF, c.byref(number), context)
            callback(name, 0xFFFFFFFF, None, context)
        callbacks['active'](1, None)
        event = Event()
        for expected_index in range(len(input_names)):
            flags = 3 if expected_index == 0 else 0
            assert callbacks['event'](c.byref(event), flags, None) == 0
            assert event.index == expected_index and event.payload[0] == 0
        assert callbacks['event'](c.byref(event), 0, None) == -4
        assert callbacks['event'](None, 1, None) == -2
        callback, context = events[3]
        callback(3, None, context)
        assert callbacks['event'](c.byref(event), 1, None) == 0 and event.payload[0] == 0
        start = time.monotonic()
        if cycle % 3 == 1:
            api.scs_telemetry_shutdown()
            api.scs_input_shutdown()
        else:
            api.scs_input_shutdown()
            api.scs_telemetry_shutdown()
        assert time.monotonic() - start < 2, 'worker did not shut down promptly'
        events.clear()
        channels.clear()
    assert not failures, failures
    print('PASS: x64 PE exports, ABI layouts, game rejection, rollback, callbacks, 6 ETS2/ATS init/shutdown cycles')


if __name__ == '__main__':
    probe(Path(sys.argv[1]).resolve())
