const { Command } = require('commander')
const { generate } = require('./generate')

const run = async () => {
    const program = new Command()
    program
        .name('CLI')
        .description('generate test cases')
        .version('1')
        .allowExcessArguments(false)


    program.command('generate')
        .alias('g')
        .description('Generate a test case')
        .argument('<wallets>', 'wallets count')
        .argument('<snaps>', 'snaps count')
        .argument('<p2p attestations>', 'p2p attestations count')
        .argument('<snap attestations>', 'snap attestations count')
        .action(async (
            walletsCount,
            snapsCount,
            p2pAttestationsCount,
            snapAttestationsCount,
        ) => {
            await generate(walletsCount,
                snapsCount,
                p2pAttestationsCount,
                snapAttestationsCount)
        })

    try {
        program.parse(process.argv)
    } catch (e) {
        l.error(e)
        process.exit(1)
    }
}

run()