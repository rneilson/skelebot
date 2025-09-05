#!/bin/bash

set -euo pipefail

hwmon_dir=""

find_hwmon_dir() {
    for dir in $(ls -d1 /sys/class/hwmon/hwmon*); do
        if ! [[ -f "$dir/name" ]]; then
            continue
        fi
        if [[ $(cat "$dir/name") = "ina219" ]]; then
            hwmon_dir=$dir
            break
        fi
    done
}

get_voltage() {
    echo "scale=2; $(cat ${hwmon_dir}/in1_input) / 1000" | bc
}

get_current() {
    cat "${hwmon_dir}/curr1_input"
}

get_power() {
    echo "scale=2; $(cat ${hwmon_dir}/power1_input) / 1000000" | bc
}

main() {
    find_hwmon_dir
    if [[ -z "$hwmon_dir" ]]; then
        echo "No INA219 hwmon sysfs dir found!" >&2
        exit 1
    fi

    # TODO: determine color for icon based on battery voltage

    echo "<txt> $(get_voltage) V | $(get_current) mA | $(get_power) W </txt>"
}

main "$@"
