function getCookie(name) {
  return document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${name}=`))
    ?.slice(name.length + 1);
}

async function request(method, path, body) {
  const headers = {};
  const mutates = !["GET", "HEAD", "OPTIONS"].includes(method.toUpperCase());
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (mutates) {
    const xsrf = getCookie("xsrf-token");
    if (xsrf) headers["x-xsrf-token"] = decodeURIComponent(xsrf);
  }

  const response = await fetch(path, {
    method,
    credentials: "include",
    headers,
    body: body === undefined ? undefined : JSON.stringify(body)
  });

  const text = await response.text();
  let data = null;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }
  }
  if (!response.ok) {
    const errorMessage =
      typeof data === "string" ? data : data?.error || `Request failed: ${response.status}`;
    const error = new Error(errorMessage);
    error.status = response.status;
    error.data = data;
    throw error;
  }
  return data;
}

let refreshPromise = null;

function refreshTokens() {
  if (!refreshPromise) {
    refreshPromise = request("GET", "/api/auth/csrf-token")
      .then(() => request("POST", "/api/auth/refresh", {}))
      .finally(() => {
        refreshPromise = null;
      });
  }
  return refreshPromise;
}

async function authenticatedRequest(method, path, body) {
  try {
    return await request(method, path, body);
  } catch (error) {
    if (error.status !== 401) throw error;
    await refreshTokens();
    return request(method, path, body);
  }
}

export const api = {
  ensureCsrf: () => request("GET", "/api/auth/csrf-token"),
  landing: () => request("GET", "/api/public/landing"),
  listUpcomingStudyTopics: () => request("GET", "/api/public/study-topics/upcoming"),
  session: () => authenticatedRequest("GET", "/api/auth/session"),
  signUp: (payload) => request("POST", "/api/auth/sign-up", payload),
  signIn: (payload) => request("POST", "/api/auth/sign-in", payload),
  refresh: () => refreshTokens(),
  logout: () => request("POST", "/api/auth/logout", {}),
  changePassword: (payload) => authenticatedRequest("POST", "/api/auth/change-password", payload),
  deleteOwnAccount: (payload) => authenticatedRequest("DELETE", "/api/auth/delete-account", payload),
  listPosts: (page = 1, pageSize = 20) =>
    authenticatedRequest("GET", `/api/feed/posts?page=${page}&page_size=${pageSize}`),
  getThread: (postId) => authenticatedRequest("GET", `/api/feed/posts/${postId}`),
  createPost: (payload, anonymous = false) =>
    authenticatedRequest(
      "POST",
      anonymous ? "/api/feed/posts?anonymous=true" : "/api/feed/posts",
      payload
    ),
  createSurveyResponse: (payload) => request("POST", "/api/survey-responses", payload),
  createReply: (postId, payload) =>
    authenticatedRequest("POST", `/api/feed/posts/${postId}/replies`, payload),
  deletePost: (postId) => authenticatedRequest("DELETE", `/api/feed/posts/${postId}`),
  deleteReply: (replyId) => authenticatedRequest("DELETE", `/api/feed/replies/${replyId}`),
  listNotifications: () => authenticatedRequest("GET", "/api/notifications?page=1&page_size=100"),
  markNotificationsRead: (ids) => authenticatedRequest("POST", "/api/notifications/read", { ids }),
  clearNotifications: () => authenticatedRequest("DELETE", "/api/notifications/clear", {}),
  listPending: () => authenticatedRequest("GET", "/api/admin/pending"),
  listPendingAnonymousPosts: () =>
    authenticatedRequest("GET", "/api/admin/anonymous-posts/pending"),
  listSurveyResponses: (page = 1, pageSize = 25) =>
    authenticatedRequest(
      "GET",
      `/api/admin/survey-responses?page=${page}&page_size=${pageSize}`
    ),
  listUsers: () => authenticatedRequest("GET", "/api/admin/users?page=1&page_size=200"),
  approveUser: (userId) => authenticatedRequest("POST", `/api/admin/users/${userId}/approve`, {}),
  approveAnonymousPost: (postId) =>
    authenticatedRequest("POST", `/api/admin/anonymous-posts/${postId}/approve`, {}),
  setAdmin: (userId, is_admin) =>
    authenticatedRequest("POST", `/api/admin/users/${userId}/role`, { is_admin }),
  setUserStatus: (userId, status) =>
    authenticatedRequest("POST", `/api/admin/users/${userId}/status`, { status }),
  resetPassword: (userId) =>
    authenticatedRequest("POST", `/api/admin/users/${userId}/reset-password`, {}),
  deleteUser: (userId) => authenticatedRequest("DELETE", `/api/admin/users/${userId}`, {}),
  deleteContent: (kind, id) =>
    authenticatedRequest("DELETE", `/api/admin/content/${kind}/${id}`, {}),
  listEvents: () => authenticatedRequest("GET", "/api/admin/events"),
  createEvent: (payload) => authenticatedRequest("POST", "/api/admin/events", payload),
  updateEvent: (eventId, payload) =>
    authenticatedRequest("PATCH", `/api/admin/events/${eventId}`, payload),
  deleteEvent: (eventId) => authenticatedRequest("DELETE", `/api/admin/events/${eventId}`, {}),
  listStudyTopics: () => authenticatedRequest("GET", "/api/admin/study-topics"),
  createStudyTopic: (payload) => authenticatedRequest("POST", "/api/admin/study-topics", payload),
  updateStudyTopic: (topicId, payload) =>
    authenticatedRequest("PATCH", `/api/admin/study-topics/${topicId}`, payload),
  deleteStudyTopic: (topicId) =>
    authenticatedRequest("DELETE", `/api/admin/study-topics/${topicId}`, {})
};
