#!/bin/sh

set -eu

unset -v progname progdir
progname="${0##*/}"
case "${0}" in
	*/*) progdir="${0%/*}";;
	*) progdir=.;;
esac
msg() { case $# in [1-9]*) echo "${progname}: $*" >&2;; esac; }
err() { local code="${1:-1}"; shift || : 2> /dev/null; msg "$@"; exit "${code}"; }

case $# in
	0) err 64 "specify the input CSV file";;
	1) err 64 "specify the pre-trust file";;
esac
unset -v csv pt
csv="${1}"
pt="${2}"
shift 2
[ -f "${csv}" ] || err 66 "cannot find input CSV file ${csv}"
[ -f "${pt}" ] || err 66 "cannot find pre-trust file ${pt}"

cd "${progdir}" || exit
which cargo > /dev/null 2>&1 || err 69 "cargo not found; install Rust from https://rustup.rs"
which go > /dev/null 2>&1 || err 69 "go not found, install Go, e.g. using https://github.com/travis-ci/gimme"
case "${GOBIN+set}"  in
	"")
		GOBIN="${GOPATH-"${HOME}/go"}/bin"
		;;
esac
export GOBIN
mkdir -p "${GOBIN}"

msg "installing eigentrust into ${GOBIN}"
go install k3l.io/go-eigentrust/cmd/eigentrust@grpc
[ -f "${GOBIN}/eigentrust" -a -x "${GOBIN}/eigentrust" ] || \
	err 69 "cannot install go-eigentrust binary"

msg "installing grpcurl into ${GOBIN}"
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
[ -f "${GOBIN}/grpcurl" -a -x "${GOBIN}/grpcurl" ] || \
	err 69 "cannot install grpcurl binary"

msg "building all Rust pipeline binaries"
cargo build --all || exit
unset -v binary
for binary in spd-tv indexer attestation-transformer linear-combiner job-manager snap-score-computer; do
	[ -f "target/debug/${binary}" -a -x "target/debug/${binary}" ] || \
		err 69 "target/debug/${binary} missing or not executable"
done

unset -v pids sig pid
pids=""

cleanup() {
	msg "terminating PIDS:${pids-" (none)"}"
	kill ${pids-} || :
	msg "waiting for jobs to terminate"
	wait
}

trap 'cleanup' EXIT
for sig in HUP INT TERM
do
	trap 'trap - '"${sig}"'; cleanup; kill -'"${sig}"' $$' "${sig}"
done

rm -rf db cache lc-storage att-tr-storage

msg "running components in the background"

"${GOBIN}/eigentrust" grpc > et.log 2>&1 & pid=$!
msg "go-eigentrust pid=${pid}"
pids="${pids} ${pid}"

_grpcurl() { # proto_file [host:]port call [data]
	local proto_file host_port call data
	proto_file="${1?}" || return
	host_port="${2?}" || return
	call="${3?}" || return
	data="${4-}"
	case "${host_port}" in
		*:*) ;;
		*) host_port="[::1]:${host_port}";;
	esac
	"${GOBIN}/grpcurl" -import-path trustvector/api/pb -import-path trustmatrix/api/pb -import-path compute/api/pb -import-path proto-buf/services -proto "${proto_file}" -d "${data}" -plaintext "${host_port}" "${call}"
}

is_et_ready() {
	case $(_grpcurl trustvector.proto 8080 trustvector.Service.Create | jq .id) in
		?*) return 0;;
	esac
	return 1
}

while ! is_et_ready
do
	msg "waiting for go-eigentrust gRPC to become ready"
	sleep 1
done

msg "initializing trust vectors"
unset -v v
for v in pt gt1 gt2 gtp1 gtp2; do
	target/debug/spd-tv create --id="${v}" || target/debug/spd-tv flush --id="${v}"
done
target/debug/indexer --csv "${csv}" > idx.log 2>&1 & pid=$!
msg "indexer pid=${pid}"
pids="${pids} ${pid}"

SPD_LC_LOG="linear_combiner=trace" target/debug/linear-combiner > lc.log 2>&1 & pid=$!
msg "linear-combiner pid=${pid}"
pids="${pids} ${pid}"

SPD_AT_LOG="attestation_transformer=trace" target/debug/attestation-transformer > at.log 2>&1 & pid=$!
msg "attestation-transformer pid=${pid}"
pids="${pids} ${pid}"

while ! _grpcurl indexer.proto 50050 indexer.Indexer/Subscribe
do
	msg "waiting for indexer gRPC to become ready"
	sleep 1
done
while ! _grpcurl combiner.proto 50052 combiner.LinearCombiner/GetDidMapping
do
	msg "waiting for linear-combiner gRPC to become ready"
	sleep 1
done
while ! _grpcurl transformer.proto 50051 transformer.Transformer/TermStream
do
	msg "waiting for attestation-transformer gRPC to become ready"
	sleep 1
done

SPD_JM_LOG="job_manager=trace" target/debug/job-manager > jm.log 2>&1 & pid=$!
msg "job-manager pid=${pid}"
pids="${pids} ${pid}"

target/debug/snap-score-computer --log-level=debug --interval=120000 > ssc.log 2>&1 & pid=$!
msg "snap-score-computer pid=${pid}"
pids="${pids} ${pid}"

msg "all components launched (pids${pids})"
msg "check spd_scores/ and *.log"
msg "press ^C to stop the pipeline"
sleep 31536000
