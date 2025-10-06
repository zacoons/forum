const tls = undefined;
const corsHeaders = {
	"Access-Control-Allow-Origin": "*",
	"Access-Control-Allow-Methods": "GET, POST",
	"Access-Control-Allow-Headers": "Content-Type",
};

const usersPromise = Bun.file("users.json").json();

const authTokens: any = {};

Bun.serve({
	port: process.env.PORT || 8080,
	tls,

	routes: {
		// TODO: entirely separate frontend impl from backend impl
		"/": new Response(await Bun.file("../frontend/index.html").bytes()),
		"/login": new Response(await Bun.file("../frontend/login.html").bytes()),

		"/_forum/index": {
			GET: async req => {
				// Check if user is auth'd
				const username = req.cookies.get("username");
				const authTok = req.cookies.get("authTok");
				if (!username || !authTok || authTokens[username] !== authTok) {
					return new Response(null, { status: 401, headers: corsHeaders });
				}

				const threads = [
					{
						"id": "asduy910872dhs",
						"title": "beans",
						"msg": "Does anyone here like beans? I love beans in chile-con-carne."
					},
				];
				return Response.json(threads, { status: 200, headers: corsHeaders });
			}
		},

		"/_forum/auth": {
			POST: async req => {
				// Parse relevant data
				if (!req.body) {
					return new Response("Request body was null", { status: 400, headers: corsHeaders });
				}
				const body = await req.body.text();
				const items = body.split("\0", 2);
				if (items.length !== 2 || !items[0] || !items[1]) {
					return new Response("Bad format for request body", { status: 400, headers: corsHeaders });
				}
				const [username, passwd] = items;
				const users = await usersPromise;
				const user = users[username];
				if (!user || !user.password) {
					return new Response(null, { status: 401, headers: corsHeaders });
				}

				// Verify password
				const isMatch = await Bun.password.verify(passwd, user.password);
				if (!isMatch) {
					return new Response(null, { status: 401, headers: corsHeaders });
				}

				// Grant auth token
				const authTok = Bun.randomUUIDv7();
				authTokens[username] = authTok;
				req.cookies.set("username", username);
				req.cookies.set("authTok", authTok);
				return new Response(null, { status: 200, headers: corsHeaders });
			},
		},
	},
});
