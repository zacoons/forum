const passwds = [];
process.stdout.write("Password: ");
for await (const line of console) {
	if (!line) {
		continue;
	}
	passwds.push(line);
	if (passwds.length === 1) {
		process.stdout.write("Confirm: ");
	}
	else if (passwds.length === 2) {
		break;
	}
}
if (passwds[0] !== passwds[1]) {
	process.stderr.write("Mismatching passwords\n");
	process.exit(1);
}
const hash = await Bun.password.hash(passwds[0]!);
process.stdout.write(hash + "\n");
