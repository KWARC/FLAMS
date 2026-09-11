#![allow(rustdoc::private_intra_doc_links)]
#![allow(unused_imports)]
/*!
 *
 * # Public API Endpoints
 *
 * ## Encoding
 *
 * **POST** requests have their arguments `application/x-www-form-urlencoded`-encoded, unless otherwise
 * specified.
 *
 * **GET** requests have their arguments url-encoded. Endpoints that take a
 * (particular kind of) [URI] have those represented via the following encoding:
 * - Either `uri=<STRING>` ( a full [URI]), or
 * - `a=<STRING>&rp=<STRING>` (an [ArchiveId] and a relative path to a source file in the archive including file extension)
 *   can be used for  [DocumentUri]s, or
 * - the [URI] components with relevant argument names; e.g. for a [DocumentUri]:
 *   `?a=<STRING>[&p=<STRING>]&l=<LANGUAGE>&d=<NAME>`.
 *
 * ## Endpoints
 *
 * | Path         | GET/POST | Arguments | Description / Return Value |
 * | ------------ | --- | --------- | ----------- |
 * | [`/api/index`](backend::index()) | POST | (None) |  |
 * | [`/api/settings`](server_fns::settings) | POST | (None) | [Settings](SettingsSpec) (requires admin login) |
 * | [`/api/reload`](server_fns::reload) | POST | (None) | (requires admin login) |
 * | [`/api/login`](login) | POST | `username=<STRING>`, `password=<STRING>` | log in |
 * | [`/api/login_state`](login_state) | POST | (None) | [LoginState] |
 * | [`/api/search`](search::search_query) | POST | `query=<STRING>&opts=`[`QueryFilter`](flams_ontology::search::QueryFilter)`&num_results=<INT>` | `Vec<(<FLOAT>,`[`SearchResult`](flams_ontology::search::SearchResult)`)>` |
 * | [`/api/search_symbols`](search::search_symbols) | POST | `query=<STRING>&num_results=<INT>` | `Vec<(`[`SymbolUri`]`Vec<(<FLOAT>,`[`SearchResult`](flams_ontology::search::SearchResult)`)>)>` |
 * | [`/content/grade`](content::grade()) | POST | TODO |
 * | [`/content/grade_enc`](content::grade_enc()) | POST | TODO |
 * | `/gitlab_login` | POST | | |
 * | **Backend** | | | |
 * | [`/api/backend/group_entries`](backend::group_entries) | POST | (optional) `in=<STRING>` | `(Vec<`[ArchiveGroupData](crate::router::backend::ArchiveGroupData)`>,Vec<`[ArchiveData](crate::router::backend::ArchiveData)`>)` - the archives and archive groups in the provided archive group (if given) or on the top-level (if None) |
 * | [`/api/backend/archive_entries`](backend::archive_entries) | POST | `archive=<STRING>`, (optional) `path=<STRING>` | `(Vec<`[DirectoryData](crate::router::backend::DirectoryData)`>,Vec<`[FileData](crate::router::backend::FileData)`>)` - the source directories and files in the provided archive, or (if given) the relative path within the provided archive |
 * | [`/api/backend/build_status`](backend::build_status) | POST | `archive=<STRING>`, (optional) `path=<STRING>` | [FileStates](crate::router::backend::FileStates) - the build status of the provided archive, or (if given) the relative path within the provided archive (requires admin login) |
 * | [`/api/backend/query`](query_api) | `query=<STRING>` | POST | `STRING` - SPARQL query endpoint; returns SPARQL JSON |
 * | [`/api/backend/archive_dependencies`](backend::archive_dependencies) | POST | `archives=Vec<STRING>` | `Vec<`[`ArchiveId`]`>` |
 * | [`/api/backend/source_file`](backend::source_file) | POST | [URI] | `STRING` - Returns the git URL of the source file for the given URL |
 * | **Build Queue** | | |
 * | [`/api/buildqueue/enqueue`](buildqueue::enqueue) | POST | `archive=<STRING>`,  `target=<`[FormatOrTarget](crate::router::backend::FormatOrTarget)`>`, (optional) `path=STRING`, (optional) `stale_only=<BOOL>` (default:true) | `usize` - enqueue a new build job. Returns number of jobs queued (requires admin login)|
 * | [`/api/buildqueue/get_queues`](buildqueue::get_queues) | POST | | `Vec<(NonZeroU32,String)>` - return the list of all (waiting or running) build queues as (id,name) pairs (requires admin login)|
 * | [`/api/buildqueue/run`](buildqueue::run) | POST | `id=<NonZeroU32>` | runs the build queue with the given id (requires admin login)|
 * | [`/api/buildqueue/requeue`](buildqueue::requeue) | POST | `id=<NonZeroU32>` | requeues failed tasks in the queue with the given id (requires admin login)|
 * | [`/api/buildqueue/log`](buildqueue::get_log) | POST | `archive=<STRING>`, `rel_path=<STRING>`, `target=<STRING>` | returns the log of the stated build job (requires admin login)|
 * | [`/api/buildqueue/migrate`](buildqueue::migrate) | POST | `id=<NonZeroU32>` | |
 * | [`/api/buildqueue/delete`](buildqueue::delete) | POST | `id=<NonZeroU32>` | |
 * | **Git** | | |
 * | [`/api/gitlab/get_archives`](git::get_archives) | POST | |  - returns the list of GitLab projects |
 * | [`/api/gitlab/get_branches`](git::get_branches) | POST | `id=<u64>` | `Vec<`[`Branch`](flams_git::Branch)`>` - returns the list of branches for the given GitLab project |
 * | [`/api/gitlab/get_new_commits`](git::get_new_commit) | POST | `queue=<u64>&id=ArchiveId` | `Vec<`(String,`[`Commit`](flams_git::Commit)`)`>` |
 * | | **Web Sockets** | | |
 * | [`/ws/log`](crate::router::logging::LogSocket) | |  |  |
 * | [`/ws/queue`](crate::router::buildqueue::QueueSocket) | | |  |
 * | **Content** |  | | |
 * | [`/img`](img_handler) | GET | `kpse=<STRING>` or `file=<STRING>` (LSP only) or `a=<ArchiveID>&rp=<STRING>` | Images |
 * | [`/content/document`](content::document) | GET | [DocumentUri] | `(`[DocumentUri],`Vec<`[CSS]`>,String)` Returns a pair of CSS rules and the full body of the HTML for the given document (with the `<body>` node replaced by a `<div>`, but preserving all attributes/classes) |
 * | [`/content/fragment`](content::fragment) | GET | [URI]`[&context=`[URI]`]` | `(`[URI]`,Vec<`[CSS]`>,String)` Returns a pair of CSS rules and the HTML fragment representing the given element; i.e. the inner HTML of a document (for inputrefs), the HTML of a semantic paragraph, etc. |
 * | [`/content/title`](content::title) | GET | [URI] | `(Vec<`[CSS]`>,String)` Returns a pair of CSS rules and the HTML title of the given element |
 * | [`/content/omdoc`](content::omdoc) | GET | [URI] | [`AnySpec`] Returns the structural representation of the OMDoc content at the given URI |
 * | [`/content/toc`](content::toc()) | GET | [DocumentUri] | `(Vec<`[CSS]`>,Vec<`[TOCElem]`>)` Returns a pair of CSS rules and the table of contents of the given document, including section titles |
 * | [`/content/los`](content::los()) | GET | [SymbolUri] | `(Vec<(`[DocumentElementUri]`,`[LOKind]`)>` Returns a list of all Learning Objects for the given symbol |
 * | [`/content/notations`](content::notations()) | GET | [SymbolUri] | `(Vec<(`[DocumentElementUri]`,`[Notation]`)>` Returns a list of all Notations for the given symbol or variable |
 * | [`/content/solution`](content::solution()) | GET | [DocumentElementUri] | [`Solutions`](flams_ontology::narration::problems::Solutions) |
 * | [`/content/quiz`](content::get_quiz()) | GET | [DocumentUri] | [`Quiz`](flams_ontology::narration::problems::Quiz) |
 * | [`/content/slides`](content::slides_view()) | GET | [DocumentUri] or [DocumentElementUri] | `(Vec<CSS>,Vec<SlideElement>)` |
 * | [`/content/legacy/uris`](content::uris()) | GET | | |
*/

use flams_main::server::files::img_handler;

use flams_router_dashboard::{
    LoginState,
    query::query_api,
    server_fns::{
        self, backend, buildqueue, content, git,
        login::{login, login_state},
        search,
    },
};
use flams_utils::settings::SettingsSpec;
use ftml_ontology::narrative::elements::notations::Notation;
use ftml_uris::prelude::*;
