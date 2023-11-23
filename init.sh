if [[ $(cargo --version) ]]; then
    echo "Found cargo"
else
    (curl -sSf https://sh.rustup.rs || echo "exit 1") | sh -s -- -y || exit
    source $HOME/.cargo/env || exit
    export PATH=$HOME/.cargo/bin:$PATH
fi
