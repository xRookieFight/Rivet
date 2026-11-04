//! Blocking GitHub REST + GraphQL client.
//!
//! Calls are made from background threads (see `task::Load`), so blocking
//! reqwest keeps things simple. The client is cheap to clone.

pub mod models;
pub mod web;

use anyhow::{Context as _, Result, anyhow, bail};
use models::*;
use reqwest::Method;
use reqwest::blocking::{Client as Http, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const API: &str = "https://api.github.com";
const UA: &str = concat!("Rivet/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Client {
    http: Http,
    token: String,
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// Paths in the contents API keep their slashes, every segment gets encoded.
fn enc_path(p: &str) -> String {
    p.split('/').map(enc).collect::<Vec<_>>().join("/")
}

impl Client {
    pub fn new(token: impl Into<String>) -> Self {
        let http = Http::builder().user_agent(UA).timeout(std::time::Duration::from_secs(60)).build().expect("http client");
        Self { http, token: token.into() }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        let url = if path.starts_with("http") { path.to_string() } else { format!("{API}{path}") };
        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    fn check(resp: Response) -> Result<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body: Value = resp.json().unwrap_or(Value::Null);
        let mut msg = body["message"].as_str().unwrap_or("request failed").to_string();
        if let Some(errs) = body["errors"].as_array() {
            let details: Vec<String> = errs
                .iter()
                .filter_map(|e| e["message"].as_str().map(str::to_string).or_else(|| e["code"].as_str().map(str::to_string)))
                .collect();
            if !details.is_empty() {
                msg = format!("{msg}: {}", details.join(", "));
            }
        }
        bail!("GitHub returned {} ({})", status.as_u16(), msg)
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let r = Self::check(self.req(Method::GET, path).send()?)?;
        r.json().with_context(|| format!("decoding {path}"))
    }

    fn send_json<T: DeserializeOwned>(&self, method: Method, path: &str, body: &Value) -> Result<T> {
        let r = Self::check(self.req(method, path).json(body).send()?)?;
        if r.status() == reqwest::StatusCode::NO_CONTENT {
            return serde_json::from_value(Value::Null).context("empty response");
        }
        let text = r.text()?;
        if text.trim().is_empty() {
            return serde_json::from_value(Value::Null).context("empty response");
        }
        serde_json::from_str(&text).with_context(|| format!("decoding {path}"))
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.send_json(Method::POST, path, body)
    }

    pub fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.send_json(Method::PATCH, path, body)
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        self.send_json(Method::PUT, path, body)
    }

    /// For endpoints that answer 204 and we only care that it worked.
    pub fn send_empty(&self, method: Method, path: &str, body: Option<&Value>) -> Result<()> {
        let mut rb = self.req(method, path);
        if let Some(b) = body {
            rb = rb.json(b);
        } else {
            rb = rb.header("Content-Length", "0");
        }
        Self::check(rb.send()?)?;
        Ok(())
    }

    /// Returns true on 204, false on 404. GitHub uses this for "is starred?" style checks.
    fn exists(&self, path: &str) -> Result<bool> {
        let r = self.req(Method::GET, path).send()?;
        match r.status().as_u16() {
            200..=299 => Ok(true),
            404 => Ok(false),
            _ => Self::check(r).map(|_| false),
        }
    }

    pub fn graphql(&self, query: &str, vars: Value) -> Result<Value> {
        let v: Value = self.post("/graphql", &json!({ "query": query, "variables": vars }))?;
        if let Some(errs) = v["errors"].as_array().filter(|e| !e.is_empty()) {
            bail!("GraphQL: {}", errs[0]["message"].as_str().unwrap_or("error"));
        }
        Ok(v["data"].clone())
    }

    // users and orgs

    pub fn me(&self) -> Result<User> {
        self.get("/user")
    }

    pub fn user(&self, login: &str) -> Result<User> {
        self.get(&format!("/users/{}", enc(login)))
    }

    pub fn my_orgs(&self) -> Result<Vec<Org>> {
        self.get("/user/orgs?per_page=100")
    }

    pub fn received_events(&self, login: &str, page: u32) -> Result<Vec<Event>> {
        self.get(&format!("/users/{}/received_events?per_page=100&page={page}", enc(login)))
    }

    /// Logins you follow, up to 1000. The feed uses it to keep stars and forks
    /// to people you actually follow, like github's own feed.
    pub fn following(&self) -> Result<std::collections::HashSet<String>> {
        let mut out = std::collections::HashSet::new();
        for page in 1..=10 {
            let users: Vec<User> = self.get(&format!("/user/following?per_page=100&page={page}"))?;
            let n = users.len();
            out.extend(users.into_iter().map(|u| u.login));
            if n < 100 {
                break;
            }
        }
        Ok(out)
    }

    pub fn social_accounts(&self, login: &str) -> Result<Vec<SocialAccount>> {
        self.get(&format!("/users/{}/social_accounts", enc(login)))
    }

    /// Public org memberships of any user (yours include private ones via `my_orgs`).
    pub fn user_orgs(&self, login: &str) -> Result<Vec<Org>> {
        self.get(&format!("/users/{}/orgs?per_page=100", enc(login)))
    }

    pub fn starred_by(&self, login: &str, page: u32) -> Result<Vec<Repo>> {
        self.get(&format!("/users/{}/starred?per_page=30&page={page}", enc(login)))
    }

    pub fn is_following(&self, login: &str) -> Result<bool> {
        self.exists(&format!("/user/following/{}", enc(login)))
    }

    pub fn set_following(&self, login: &str, follow: bool) -> Result<()> {
        let m = if follow { Method::PUT } else { Method::DELETE };
        self.send_empty(m, &format!("/user/following/{}", enc(login)), None)
    }

    // repositories

    pub fn my_repos(&self, sort: &str, page: u32) -> Result<Vec<Repo>> {
        self.get(&format!("/user/repos?sort={sort}&per_page=50&page={page}&affiliation=owner,collaborator,organization_member"))
    }

    pub fn user_repos(&self, login: &str, page: u32) -> Result<Vec<Repo>> {
        self.get(&format!("/users/{}/repos?sort=updated&per_page=50&page={page}", enc(login)))
    }

    pub fn org_repos(&self, org: &str, page: u32) -> Result<Vec<Repo>> {
        self.get(&format!("/orgs/{}/repos?sort=updated&per_page=50&page={page}", enc(org)))
    }

    pub fn repo(&self, owner: &str, name: &str) -> Result<Repo> {
        self.get(&format!("/repos/{}/{}", enc(owner), enc(name)))
    }

    pub fn create_repo(&self, org: Option<&str>, body: &Value) -> Result<Repo> {
        match org {
            Some(o) => self.post(&format!("/orgs/{}/repos", enc(o)), body),
            None => self.post("/user/repos", body),
        }
    }

    pub fn update_repo(&self, owner: &str, name: &str, body: &Value) -> Result<Repo> {
        self.patch(&format!("/repos/{}/{}", enc(owner), enc(name)), body)
    }

    pub fn delete_repo(&self, owner: &str, name: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}", enc(owner), enc(name)), None)
    }

    pub fn replace_topics(&self, owner: &str, name: &str, topics: &[String]) -> Result<()> {
        let _: Value = self.put(&format!("/repos/{}/{}/topics", enc(owner), enc(name)), &json!({ "names": topics }))?;
        Ok(())
    }

    pub fn is_starred(&self, owner: &str, name: &str) -> Result<bool> {
        self.exists(&format!("/user/starred/{}/{}", enc(owner), enc(name)))
    }

    pub fn set_starred(&self, owner: &str, name: &str, star: bool) -> Result<()> {
        let m = if star { Method::PUT } else { Method::DELETE };
        self.send_empty(m, &format!("/user/starred/{}/{}", enc(owner), enc(name)), None)
    }

    /// "watching" | "ignoring" | "participating" (no subscription).
    pub fn subscription(&self, owner: &str, name: &str) -> Result<String> {
        let r = self.req(Method::GET, &format!("/repos/{}/{}/subscription", enc(owner), enc(name))).send()?;
        if r.status().as_u16() == 404 {
            return Ok("participating".into());
        }
        let v: Value = Self::check(r)?.json()?;
        Ok(if v["ignored"].as_bool() == Some(true) { "ignoring" } else { "watching" }.into())
    }

    pub fn set_subscription(&self, owner: &str, name: &str, mode: &str) -> Result<()> {
        let path = format!("/repos/{}/{}/subscription", enc(owner), enc(name));
        match mode {
            "watching" => self.put::<Value>(&path, &json!({ "subscribed": true })).map(|_| ()),
            "ignoring" => self.put::<Value>(&path, &json!({ "ignored": true })).map(|_| ()),
            _ => self.send_empty(Method::DELETE, &path, None),
        }
    }

    pub fn fork(&self, owner: &str, name: &str, org: Option<&str>, new_name: Option<&str>) -> Result<Repo> {
        let mut body = json!({ "default_branch_only": true });
        if let Some(o) = org {
            body["organization"] = json!(o);
        }
        if let Some(n) = new_name {
            body["name"] = json!(n);
        }
        self.post(&format!("/repos/{}/{}/forks", enc(owner), enc(name)), &body)
    }

    pub fn contents(&self, owner: &str, name: &str, path: &str, git_ref: &str) -> Result<Vec<Content>> {
        let url = format!("/repos/{}/{}/contents/{}?ref={}", enc(owner), enc(name), enc_path(path), enc(git_ref));
        let v: Value = self.get(&url)?;
        if v.is_array() { Ok(serde_json::from_value(v)?) } else { Ok(vec![serde_json::from_value(v)?]) }
    }

    pub fn file(&self, owner: &str, name: &str, path: &str, git_ref: &str) -> Result<Content> {
        let url = format!("/repos/{}/{}/contents/{}?ref={}", enc(owner), enc(name), enc_path(path), enc(git_ref));
        self.get(&url)
    }

    /// Like `file` but None on 404, for "does this repo have X" checks.
    pub fn file_opt(&self, owner: &str, name: &str, path: &str, git_ref: &str) -> Result<Option<Content>> {
        match self.file(owner, name, path, git_ref) {
            Ok(c) => Ok(Some(c)),
            Err(e) if e.to_string().contains("404") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Files bigger than 1MB come back without content, so grab the blob instead.
    pub fn blob_text(&self, owner: &str, name: &str, sha: &str) -> Result<Vec<u8>> {
        let v: Content = self.get(&format!("/repos/{}/{}/git/blobs/{}", enc(owner), enc(name), sha))?;
        v.decoded().ok_or_else(|| anyhow!("blob could not be decoded"))
    }

    pub fn readme(&self, owner: &str, name: &str, git_ref: &str) -> Result<Option<Content>> {
        let r = self.req(Method::GET, &format!("/repos/{}/{}/readme?ref={}", enc(owner), enc(name), enc(git_ref))).send()?;
        if r.status().as_u16() == 404 {
            return Ok(None);
        }
        Ok(Some(Self::check(r)?.json()?))
    }

    /// Creates or updates a file through the contents API (one commit per call).
    pub fn put_file(
        &self,
        owner: &str,
        name: &str,
        path: &str,
        content: &[u8],
        message: &str,
        branch: &str,
        sha: Option<&str>,
    ) -> Result<()> {
        use base64::Engine;
        let mut body = json!({
            "message": message,
            "content": base64::engine::general_purpose::STANDARD.encode(content),
            "branch": branch,
        });
        if let Some(s) = sha {
            body["sha"] = json!(s);
        }
        let _: Value = self.put(&format!("/repos/{}/{}/contents/{}", enc(owner), enc(name), enc_path(path)), &body)?;
        Ok(())
    }

    pub fn delete_file(&self, owner: &str, name: &str, path: &str, message: &str, branch: &str, sha: &str) -> Result<()> {
        let body = json!({ "message": message, "sha": sha, "branch": branch });
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}/contents/{}", enc(owner), enc(name), enc_path(path)), Some(&body))
    }

    pub fn branches(&self, owner: &str, name: &str) -> Result<Vec<Branch>> {
        self.get(&format!("/repos/{}/{}/branches?per_page=100", enc(owner), enc(name)))
    }

    pub fn tags(&self, owner: &str, name: &str) -> Result<Vec<Tag>> {
        self.get(&format!("/repos/{}/{}/tags?per_page=100", enc(owner), enc(name)))
    }

    pub fn releases(&self, owner: &str, name: &str) -> Result<Vec<Release>> {
        self.get(&format!("/repos/{}/{}/releases?per_page=20", enc(owner), enc(name)))
    }

    pub fn create_branch(&self, owner: &str, name: &str, branch: &str, from_sha: &str) -> Result<()> {
        let _: Value = self.post(
            &format!("/repos/{}/{}/git/refs", enc(owner), enc(name)),
            &json!({ "ref": format!("refs/heads/{branch}"), "sha": from_sha }),
        )?;
        Ok(())
    }

    pub fn delete_branch(&self, owner: &str, name: &str, branch: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}/git/refs/heads/{}", enc(owner), enc(name), enc_path(branch)), None)
    }

    pub fn commits(&self, owner: &str, name: &str, git_ref: &str, path: Option<&str>, page: u32) -> Result<Vec<Commit>> {
        let mut url = format!("/repos/{}/{}/commits?sha={}&per_page=35&page={page}", enc(owner), enc(name), enc(git_ref));
        if let Some(p) = path.filter(|p| !p.is_empty()) {
            url.push_str(&format!("&path={}", enc(p)));
        }
        self.get(&url)
    }

    pub fn commit(&self, owner: &str, name: &str, sha: &str) -> Result<Commit> {
        self.get(&format!("/repos/{}/{}/commits/{}", enc(owner), enc(name), enc(sha)))
    }

    pub fn compare(&self, owner: &str, name: &str, base: &str, head: &str) -> Result<Compare> {
        self.get(&format!("/repos/{}/{}/compare/{}...{}", enc(owner), enc(name), enc(base), enc(head)))
    }

    /// Bytes of code per language, already sorted by GitHub (largest first).
    pub fn languages(&self, owner: &str, name: &str) -> Result<Vec<(String, u64)>> {
        let v: serde_json::Map<String, Value> = self.get(&format!("/repos/{}/{}/languages", enc(owner), enc(name)))?;
        let mut out: Vec<(String, u64)> = v.into_iter().map(|(k, v)| (k, v.as_u64().unwrap_or(0))).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(out)
    }

    pub fn community_profile(&self, owner: &str, name: &str) -> Result<Value> {
        self.get(&format!("/repos/{}/{}/community/profile", enc(owner), enc(name)))
    }

    pub fn branch_protection(&self, owner: &str, name: &str, branch: &str) -> Result<Option<BranchProtection>> {
        let r = self.req(Method::GET, &format!("/repos/{}/{}/branches/{}/protection", enc(owner), enc(name), enc(branch))).send()?;
        if r.status().as_u16() == 404 {
            return Ok(None);
        }
        Ok(Some(Self::check(r)?.json()?))
    }

    pub fn set_branch_protection(&self, owner: &str, name: &str, branch: &str, body: &Value) -> Result<()> {
        let _: Value = self.put(&format!("/repos/{}/{}/branches/{}/protection", enc(owner), enc(name), enc(branch)), body)?;
        Ok(())
    }

    pub fn remove_branch_protection(&self, owner: &str, name: &str, branch: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}/branches/{}/protection", enc(owner), enc(name), enc(branch)), None)
    }

    pub fn license_templates(&self) -> Result<Vec<LicenseTemplate>> {
        self.get("/licenses")
    }

    pub fn license_template(&self, key: &str) -> Result<LicenseTemplate> {
        self.get(&format!("/licenses/{}", enc(key)))
    }

    pub fn gitignore_templates(&self) -> Result<Vec<String>> {
        self.get("/gitignore/templates")
    }

    pub fn gitignore_template(&self, name: &str) -> Result<String> {
        let v: Value = self.get(&format!("/gitignore/templates/{}", enc(name)))?;
        Ok(v["source"].as_str().unwrap_or_default().to_string())
    }

    // issues

    pub fn issue(&self, owner: &str, name: &str, number: u64) -> Result<Issue> {
        self.get(&format!("/repos/{}/{}/issues/{number}", enc(owner), enc(name)))
    }

    pub fn issue_comments(&self, owner: &str, name: &str, number: u64) -> Result<Vec<Comment>> {
        self.get(&format!("/repos/{}/{}/issues/{number}/comments?per_page=100", enc(owner), enc(name)))
    }

    pub fn create_issue(&self, owner: &str, name: &str, body: &Value) -> Result<Issue> {
        self.post(&format!("/repos/{}/{}/issues", enc(owner), enc(name)), body)
    }

    pub fn update_issue(&self, owner: &str, name: &str, number: u64, body: &Value) -> Result<Issue> {
        self.patch(&format!("/repos/{}/{}/issues/{number}", enc(owner), enc(name)), body)
    }

    pub fn add_comment(&self, owner: &str, name: &str, number: u64, body: &str) -> Result<Comment> {
        self.post(&format!("/repos/{}/{}/issues/{number}/comments", enc(owner), enc(name)), &json!({ "body": body }))
    }

    pub fn edit_comment(&self, owner: &str, name: &str, id: u64, body: &str) -> Result<Comment> {
        self.patch(&format!("/repos/{}/{}/issues/comments/{id}", enc(owner), enc(name)), &json!({ "body": body }))
    }

    pub fn delete_comment(&self, owner: &str, name: &str, id: u64) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}/issues/comments/{id}", enc(owner), enc(name)), None)
    }

    /// `target` is "issues/{n}" or "issues/comments/{id}".
    pub fn react(&self, owner: &str, name: &str, target: &str, content: &str) -> Result<()> {
        let _: Value = self.post(&format!("/repos/{}/{}/{target}/reactions", enc(owner), enc(name)), &json!({ "content": content }))?;
        Ok(())
    }

    pub fn labels(&self, owner: &str, name: &str) -> Result<Vec<Label>> {
        self.get(&format!("/repos/{}/{}/labels?per_page=100", enc(owner), enc(name)))
    }

    pub fn create_label(&self, owner: &str, name: &str, label: &str, color: &str, desc: &str) -> Result<Label> {
        self.post(
            &format!("/repos/{}/{}/labels", enc(owner), enc(name)),
            &json!({ "name": label, "color": color.trim_start_matches('#'), "description": desc }),
        )
    }

    pub fn update_label(&self, owner: &str, name: &str, old: &str, label: &str, color: &str, desc: &str) -> Result<Label> {
        self.patch(
            &format!("/repos/{}/{}/labels/{}", enc(owner), enc(name), enc(old)),
            &json!({ "new_name": label, "color": color.trim_start_matches('#'), "description": desc }),
        )
    }

    pub fn delete_label(&self, owner: &str, name: &str, label: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/repos/{}/{}/labels/{}", enc(owner), enc(name), enc(label)), None)
    }

    pub fn milestones(&self, owner: &str, name: &str) -> Result<Vec<Milestone>> {
        self.get(&format!("/repos/{}/{}/milestones?state=open&per_page=100", enc(owner), enc(name)))
    }

    pub fn assignees(&self, owner: &str, name: &str) -> Result<Vec<User>> {
        self.get(&format!("/repos/{}/{}/assignees?per_page=100", enc(owner), enc(name)))
    }

    // pull requests

    pub fn pull(&self, owner: &str, name: &str, number: u64) -> Result<Pull> {
        self.get(&format!("/repos/{}/{}/pulls/{number}", enc(owner), enc(name)))
    }

    pub fn pull_files(&self, owner: &str, name: &str, number: u64) -> Result<Vec<ChangedFile>> {
        self.get(&format!("/repos/{}/{}/pulls/{number}/files?per_page=100", enc(owner), enc(name)))
    }

    pub fn pull_commits(&self, owner: &str, name: &str, number: u64) -> Result<Vec<Commit>> {
        self.get(&format!("/repos/{}/{}/pulls/{number}/commits?per_page=100", enc(owner), enc(name)))
    }

    pub fn reviews(&self, owner: &str, name: &str, number: u64) -> Result<Vec<Review>> {
        self.get(&format!("/repos/{}/{}/pulls/{number}/reviews?per_page=100", enc(owner), enc(name)))
    }

    pub fn review_comments(&self, owner: &str, name: &str, number: u64) -> Result<Vec<ReviewComment>> {
        self.get(&format!("/repos/{}/{}/pulls/{number}/comments?per_page=100", enc(owner), enc(name)))
    }

    pub fn create_pull(&self, owner: &str, name: &str, body: &Value) -> Result<Pull> {
        self.post(&format!("/repos/{}/{}/pulls", enc(owner), enc(name)), body)
    }

    pub fn update_pull(&self, owner: &str, name: &str, number: u64, body: &Value) -> Result<Pull> {
        self.patch(&format!("/repos/{}/{}/pulls/{number}", enc(owner), enc(name)), body)
    }

    pub fn merge_pull(&self, owner: &str, name: &str, number: u64, method: &str, title: &str, message: &str) -> Result<()> {
        let _: Value = self.put(
            &format!("/repos/{}/{}/pulls/{number}/merge", enc(owner), enc(name)),
            &json!({ "merge_method": method, "commit_title": title, "commit_message": message }),
        )?;
        Ok(())
    }

    /// event: APPROVE | REQUEST_CHANGES | COMMENT. `comments` are line comments.
    pub fn submit_review(&self, owner: &str, name: &str, number: u64, event: &str, body: &str, comments: Vec<Value>) -> Result<()> {
        let _: Value = self.post(
            &format!("/repos/{}/{}/pulls/{number}/reviews", enc(owner), enc(name)),
            &json!({ "event": event, "body": body, "comments": comments }),
        )?;
        Ok(())
    }

    pub fn request_reviewers(&self, owner: &str, name: &str, number: u64, logins: &[String]) -> Result<()> {
        let _: Value =
            self.post(&format!("/repos/{}/{}/pulls/{number}/requested_reviewers", enc(owner), enc(name)), &json!({ "reviewers": logins }))?;
        Ok(())
    }

    pub fn remove_reviewers(&self, owner: &str, name: &str, number: u64, logins: &[String]) -> Result<()> {
        self.send_empty(
            Method::DELETE,
            &format!("/repos/{}/{}/pulls/{number}/requested_reviewers", enc(owner), enc(name)),
            Some(&json!({ "reviewers": logins })),
        )
    }

    pub fn check_runs(&self, owner: &str, name: &str, sha: &str) -> Result<Vec<CheckRun>> {
        let v: Value = self.get(&format!("/repos/{}/{}/commits/{}/check-runs?per_page=100", enc(owner), enc(name), enc(sha)))?;
        Ok(serde_json::from_value(v["check_runs"].clone()).unwrap_or_default())
    }

    // actions

    pub fn workflows(&self, owner: &str, name: &str) -> Result<Vec<Workflow>> {
        let v: Value = self.get(&format!("/repos/{}/{}/actions/workflows?per_page=100", enc(owner), enc(name)))?;
        Ok(serde_json::from_value(v["workflows"].clone()).unwrap_or_default())
    }

    pub fn runs(&self, owner: &str, name: &str, workflow: Option<u64>, page: u32) -> Result<(u64, Vec<WorkflowRun>)> {
        let path = match workflow {
            Some(id) => format!("/repos/{}/{}/actions/workflows/{id}/runs?per_page=30&page={page}", enc(owner), enc(name)),
            None => format!("/repos/{}/{}/actions/runs?per_page=30&page={page}", enc(owner), enc(name)),
        };
        let v: Value = self.get(&path)?;
        Ok((v["total_count"].as_u64().unwrap_or(0), serde_json::from_value(v["workflow_runs"].clone()).unwrap_or_default()))
    }

    pub fn run(&self, owner: &str, name: &str, id: u64) -> Result<WorkflowRun> {
        self.get(&format!("/repos/{}/{}/actions/runs/{id}", enc(owner), enc(name)))
    }

    pub fn jobs(&self, owner: &str, name: &str, run: u64) -> Result<Vec<Job>> {
        let v: Value = self.get(&format!("/repos/{}/{}/actions/runs/{run}/jobs?per_page=100", enc(owner), enc(name)))?;
        Ok(serde_json::from_value(v["jobs"].clone()).unwrap_or_default())
    }

    pub fn job_log(&self, owner: &str, name: &str, job: u64) -> Result<String> {
        // this one answers with a 302 to a signed blob url, reqwest follows it for us
        let r = Self::check(self.req(Method::GET, &format!("/repos/{}/{}/actions/jobs/{job}/logs", enc(owner), enc(name))).send()?)?;
        Ok(r.text()?)
    }

    pub fn dispatch(&self, owner: &str, name: &str, workflow: u64, git_ref: &str, inputs: Value) -> Result<()> {
        self.send_empty(
            Method::POST,
            &format!("/repos/{}/{}/actions/workflows/{workflow}/dispatches", enc(owner), enc(name)),
            Some(&json!({ "ref": git_ref, "inputs": inputs })),
        )
    }

    pub fn rerun(&self, owner: &str, name: &str, run: u64, failed_only: bool) -> Result<()> {
        let tail = if failed_only { "rerun-failed-jobs" } else { "rerun" };
        self.send_empty(Method::POST, &format!("/repos/{}/{}/actions/runs/{run}/{tail}", enc(owner), enc(name)), None)
    }

    pub fn cancel_run(&self, owner: &str, name: &str, run: u64) -> Result<()> {
        self.send_empty(Method::POST, &format!("/repos/{}/{}/actions/runs/{run}/cancel", enc(owner), enc(name)), None)
    }

    // notifications

    pub fn notifications(&self, all: bool, participating: bool, page: u32) -> Result<Vec<Notification>> {
        self.get(&format!("/notifications?all={all}&participating={participating}&per_page=50&page={page}"))
    }

    pub fn mark_thread_read(&self, id: &str) -> Result<()> {
        self.send_empty(Method::PATCH, &format!("/notifications/threads/{}", enc(id)), None)
    }

    pub fn mark_thread_done(&self, id: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/notifications/threads/{}", enc(id)), None)
    }

    pub fn mark_all_read(&self) -> Result<()> {
        self.send_empty(Method::PUT, "/notifications", Some(&json!({ "read": true })))
    }

    pub fn unsubscribe_thread(&self, id: &str) -> Result<()> {
        self.send_empty(Method::DELETE, &format!("/notifications/threads/{}/subscription", enc(id)), None)
    }

    // search

    pub fn search_repos(&self, q: &str, sort: &str, page: u32) -> Result<SearchResult<Repo>> {
        self.get(&format!("/search/repositories?q={}&sort={sort}&per_page=30&page={page}", enc(q)))
    }

    pub fn search_issues(&self, q: &str, sort: &str, page: u32) -> Result<SearchResult<Issue>> {
        self.get(&format!("/search/issues?q={}&sort={sort}&per_page=30&page={page}&advanced_search=true", enc(q)))
    }

    pub fn search_users(&self, q: &str, page: u32) -> Result<SearchResult<User>> {
        self.get(&format!("/search/users?q={}&per_page=30&page={page}", enc(q)))
    }

    pub fn search_commits(&self, q: &str, page: u32) -> Result<SearchResult<CommitSearchHit>> {
        self.get(&format!("/search/commits?q={}&per_page=30&page={page}", enc(q)))
    }

    pub fn search_code(&self, q: &str, page: u32) -> Result<SearchResult<CodeHit>> {
        let r = Self::check(
            self.req(Method::GET, &format!("/search/code?q={}&per_page=30&page={page}", enc(q)))
                .header("Accept", "application/vnd.github.text-match+json")
                .send()?,
        )?;
        Ok(r.json()?)
    }

    // graphql bits the REST api doesn't have

    pub fn pinned_repos(&self, login: &str) -> Result<Vec<PinnedRepo>> {
        let q = r#"query($login:String!){ user(login:$login){ pinnedItems(first:6, types:REPOSITORY){ nodes{ ... on Repository {
            name owner{login} description isPrivate isArchived stargazerCount forkCount primaryLanguage{name} parent{nameWithOwner} } } } } }"#;
        let d = self.graphql(q, json!({ "login": login }))?;
        let nodes = d["user"]["pinnedItems"]["nodes"].as_array().cloned().unwrap_or_default();
        Ok(nodes
            .iter()
            .map(|n| PinnedRepo {
                owner: n["owner"]["login"].as_str().unwrap_or_default().into(),
                name: n["name"].as_str().unwrap_or_default().into(),
                description: n["description"].as_str().map(str::to_string),
                language: n["primaryLanguage"]["name"].as_str().map(str::to_string),
                stars: n["stargazerCount"].as_u64().unwrap_or(0),
                forks: n["forkCount"].as_u64().unwrap_or(0),
                private: n["isPrivate"].as_bool().unwrap_or(false),
                archived: n["isArchived"].as_bool().unwrap_or(false),
                parent: n["parent"]["nameWithOwner"].as_str().map(str::to_string),
            })
            .collect())
    }

    /// Contribution graph plus the numbers behind the profile's activity overview
    /// and contribution activity sections, all from one GraphQL round trip.
    pub fn contributions(&self, login: &str) -> Result<ContributionCalendar> {
        self.contributions_between(login, None, None)
    }

    /// Same as `contributions` for a window (a calendar year, or one month for
    /// the activity timeline). `None` means github's default: the last year.
    pub fn contributions_between(&self, login: &str, from: Option<String>, to: Option<String>) -> Result<ContributionCalendar> {
        let q = r#"query($login:String!,$from:DateTime,$to:DateTime){ user(login:$login){ contributionsCollection(from:$from,to:$to){
            contributionYears
            contributionCalendar{ totalContributions weeks{ contributionDays{ date contributionCount color } } }
            totalCommitContributions totalIssueContributions totalPullRequestContributions
            totalPullRequestReviewContributions
            commitContributionsByRepository(maxRepositories:100){ repository{ nameWithOwner } contributions{ totalCount } }
            pullRequestContributionsByRepository(maxRepositories:100){ repository{ nameWithOwner } contributions{ totalCount } }
            issueContributionsByRepository(maxRepositories:100){ repository{ nameWithOwner } contributions{ totalCount } }
            pullRequestReviewContributionsByRepository(maxRepositories:100){ repository{ nameWithOwner } contributions{ totalCount } }
            repositoryContributions(first:100){ nodes{ repository{ nameWithOwner } } }
        } } }"#;
        let d = self.graphql(q, json!({ "login": login, "from": from, "to": to }))?;
        let cc = &d["user"]["contributionsCollection"];
        let cal = &cc["contributionCalendar"];
        let weeks = cal["weeks"]
            .as_array()
            .map(|ws| ws.iter().map(|w| serde_json::from_value(w["contributionDays"].clone()).unwrap_or_default()).collect())
            .unwrap_or_default();
        let by_repo = |key: &str| -> Vec<(String, u64)> {
            cc[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| {
                            (
                                x["repository"]["nameWithOwner"].as_str().unwrap_or_default().to_string(),
                                x["contributions"]["totalCount"].as_u64().unwrap_or(0),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let n = |key: &str| cc[key].as_u64().unwrap_or(0);
        Ok(ContributionCalendar {
            total: cal["totalContributions"].as_u64().unwrap_or(0),
            weeks,
            commits: n("totalCommitContributions"),
            issues: n("totalIssueContributions"),
            pulls: n("totalPullRequestContributions"),
            reviews: n("totalPullRequestReviewContributions"),
            commit_repos: by_repo("commitContributionsByRepository"),
            pull_repos: by_repo("pullRequestContributionsByRepository"),
            issue_repos: by_repo("issueContributionsByRepository"),
            review_repos: by_repo("pullRequestReviewContributionsByRepository"),
            years: cc["contributionYears"]
                .as_array()
                .map(|a| a.iter().filter_map(|y| y.as_i64().map(|y| y as i32)).collect())
                .unwrap_or_default(),
            created_repos: cc["repositoryContributions"]["nodes"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["repository"]["nameWithOwner"].as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        })
    }

    /// Card data for a batch of "owner/name" slugs in one GraphQL call. Missing
    /// or private repos just don't show up in the map instead of failing the batch.
    pub fn repo_summaries(&self, slugs: &[String]) -> Result<std::collections::HashMap<String, RepoSummary>> {
        // chunks go out in parallel; one slow batch shouldn't hold the whole feed up
        let results: Vec<Result<std::collections::HashMap<String, RepoSummary>>> = std::thread::scope(|scope| {
            let handles: Vec<_> = slugs.chunks(20).map(|chunk| scope.spawn(move || self.repo_summary_chunk(chunk))).collect();
            handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("summary thread panicked")))).collect()
        });
        let mut out = std::collections::HashMap::new();
        for r in results {
            out.extend(r?);
        }
        Ok(out)
    }

    fn repo_summary_chunk(&self, chunk: &[String]) -> Result<std::collections::HashMap<String, RepoSummary>> {
        let mut out = std::collections::HashMap::new();
        let fields: String = chunk
            .iter()
            .enumerate()
            .filter_map(|(i, slug)| {
                let (o, n) = slug.split_once('/')?;
                let (o, n) = (serde_json::to_string(o).ok()?, serde_json::to_string(n).ok()?);
                Some(format!(
                    "r{i}: repository(owner: {o}, name: {n}) {{ nameWithOwner description stargazerCount isPrivate viewerHasStarred pushedAt primaryLanguage {{ name }} owner {{ avatarUrl }} }} "
                ))
            })
            .collect();
        if fields.is_empty() {
            return Ok(out);
        }
        // hmm, deleted repos come back as null plus an error entry, so read data anyway
        let v: Value = self.post("/graphql", &json!({ "query": format!("query {{ {fields} }}") }))?;
        for (i, slug) in chunk.iter().enumerate() {
            let r = &v["data"][format!("r{i}")];
            if r.is_null() {
                continue;
            }
            out.insert(
                slug.clone(),
                RepoSummary {
                    full_name: r["nameWithOwner"].as_str().unwrap_or(slug).to_string(),
                    description: r["description"].as_str().map(str::to_string),
                    stars: r["stargazerCount"].as_u64().unwrap_or(0),
                    language: r["primaryLanguage"]["name"].as_str().map(str::to_string),
                    private: r["isPrivate"].as_bool().unwrap_or(false),
                    starred: r["viewerHasStarred"].as_bool().unwrap_or(false),
                    owner_avatar: r["owner"]["avatarUrl"].as_str().unwrap_or_default().to_string(),
                    pushed_at: r["pushedAt"].as_str().and_then(|d| d.parse().ok()),
                },
            );
        }
        Ok(out)
    }

    /// Latest commit touching each path at `git_ref`, one GraphQL call per 40
    /// paths via aliases. This is what fills the message column in file lists.
    pub fn last_commits(
        &self,
        owner: &str,
        name: &str,
        git_ref: &str,
        paths: &[String],
    ) -> Result<std::collections::HashMap<String, PathCommit>> {
        let mut out = std::collections::HashMap::new();
        for chunk in paths.chunks(40) {
            let fields: String = chunk
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    // paths go in as JSON string literals, which GraphQL accepts as-is
                    let lit = serde_json::to_string(p).unwrap_or_else(|_| "\"\"".into());
                    format!("p{i}: history(first: 1, path: {lit}) {{ nodes {{ oid messageHeadline committedDate }} }} ")
                })
                .collect();
            let q = format!(
                "query($o:String!,$n:String!,$r:String!){{ repository(owner:$o,name:$n){{ object(expression:$r){{ ... on Commit {{ {fields} }} }} }} }}"
            );
            let d = self.graphql(&q, json!({ "o": owner, "n": name, "r": git_ref }))?;
            let obj = &d["repository"]["object"];
            for (i, p) in chunk.iter().enumerate() {
                let node = &obj[format!("p{i}")]["nodes"][0];
                if let Some(oid) = node["oid"].as_str() {
                    out.insert(
                        p.clone(),
                        PathCommit {
                            sha: oid.to_string(),
                            message: node["messageHeadline"].as_str().unwrap_or_default().to_string(),
                            date: node["committedDate"].as_str().and_then(|d| d.parse().ok()),
                        },
                    );
                }
            }
        }
        Ok(out)
    }

    /// The node id lets us use GraphQL mutations REST lacks (e.g. pull request draft toggles).
    pub fn mark_ready_for_review(&self, node_id: &str) -> Result<()> {
        let q = "mutation($id:ID!){ markPullRequestReadyForReview(input:{pullRequestId:$id}){ clientMutationId } }";
        self.graphql(q, json!({ "id": node_id })).map(|_| ())
    }
}

/// Latest posts from the public GitHub changelog RSS, for the dashboard's
/// "Latest from our changelog" card. No token needed.
pub fn changelog(limit: usize) -> Result<Vec<ChangelogEntry>> {
    let body = Http::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(30))
        .build()?
        .get("https://github.blog/changelog/feed/")
        .send()?
        .error_for_status()?
        .text()?;
    Ok(parse_changelog(&body, limit))
}

pub fn parse_changelog(xml: &str, limit: usize) -> Vec<ChangelogEntry> {
    let between = |s: &str, open: &str, close: &str| -> Option<String> {
        let a = s.find(open)? + open.len();
        let b = s[a..].find(close)? + a;
        Some(s[a..b].trim().trim_start_matches("<![CDATA[").trim_end_matches("]]>").trim().to_string())
    };
    xml.split("<item>")
        .skip(1)
        .take(limit)
        .filter_map(|item| {
            let title = decode_entities(&between(item, "<title>", "</title>")?);
            let link = between(item, "<link>", "</link>")?;
            let date = between(item, "<pubDate>", "</pubDate>")
                .and_then(|d| chrono::DateTime::parse_from_rfc2822(&d).ok())
                .map(|d| d.with_timezone(&chrono::Utc));
            Some(ChangelogEntry { title, link, date })
        })
        .collect()
}

/// WordPress escapes a handful of characters in titles; undo the common ones.
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#8217;", "\u{2019}")
        .replace("&#8216;", "\u{2018}")
        .replace("&#8220;", "\u{201c}")
        .replace("&#8221;", "\u{201d}")
        .replace("&#8211;", "\u{2013}")
        .replace("&#8230;", "\u{2026}")
        .replace("&#038;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_rss() {
        let xml = "<rss><channel><title>x</title><item><title>Fix &amp; ship &#8217;em</title><link>https://github.blog/changelog/a</link><pubDate>Wed, 04 Nov 2026 00:32:19 +0000</pubDate></item><item><title><![CDATA[Second]]></title><link>b</link></item></channel></rss>";
        let e = parse_changelog(xml, 5);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].title, "Fix & ship \u{2019}em");
        assert_eq!(e[0].date.map(|d| d.format("%Y-%m-%d").to_string()).as_deref(), Some("2026-11-04"));
        assert_eq!(e[1].title, "Second");
        assert!(e[1].date.is_none());
    }
}
