import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { api } from "./api.js";

const emptySignIn = { email: "", password: "" };
const emptySignUp = { first_name: "", last_name: "", email: "", password: "" };
const emptyEventForm = {
  id: null,
  name: "",
  event_date: "",
  event_time: "",
  end_date: "",
  end_time: "",
  location: "",
  description: "",
};
const emptyStudyTopicForm = {
  id: null,
  week_start: "",
  name: "",
  description: "",
  hyperlink: "",
};
const emptySurveyForm = {
  heart: "",
  anonymous: true,
  food_suggestions: "",
  dietary_restrictions: "",
};
const FEED_PAGE_SIZE = 20;
const SURVEY_RESPONSE_PAGE_SIZE = 25;
const HIGHLIGHT_CLEAR_MS = 4500;
const POST_RATE_LIMIT_MESSAGE =
  "You already made a post. Please wait a few minutes before making another.";
const STUDY_WEEK_START_MESSAGE =
  "Please choose the Monday that starts the study week";

function selectorValue(value) {
  if (window.CSS?.escape) return window.CSS.escape(value);
  return value.replace(/["\\]/g, "\\$&");
}

function statusCopy(status) {
  if (status === "pending_approval") {
    return "Your account is awaiting approval from a member of the EQ presidency";
  }
  if (status === "locked") {
    return "Your account has been locked by a member of the EQ presidency";
  }
  if (status === "suspended") {
    return "Your account has been suspended from posting and replying by a member of the EQ presidency";
  }
  return "";
}

function formatEventDate(event) {
  const startDate = event.event_date;
  const startTime = event.event_time;
  const endDate = event.end_date || event.event_date;
  const endTime = event.end_time;
  const sameDay = startDate === endDate;

  if (sameDay) {
    const dateLabel = formatDateOnly(startDate);
    if (startTime && endTime) {
      return `${dateLabel}, ${formatTimeOnly(startTime)} - ${formatTimeOnly(
        endTime,
      )}`;
    }
    if (startTime) return `${dateLabel}, ${formatTimeOnly(startTime)}`;
    if (endTime) return `${dateLabel}, until ${formatTimeOnly(endTime)}`;
    return dateLabel;
  }

  const startLabel = formatEventEndpoint(startDate, startTime);
  const endLabel = formatEventEndpoint(endDate, endTime);
  return `${startLabel} - ${endLabel}`;
}

function formatDateOnly(value) {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString([], { dateStyle: "medium" });
}

function formatTimeOnly(value) {
  const date = new Date(`2000-01-01T${value}:00`);
  return date.toLocaleTimeString([], { timeStyle: "short" });
}

function formatEventEndpoint(dateValue, timeValue) {
  const value = timeValue
    ? `${dateValue}T${timeValue}:00`
    : `${dateValue}T00:00:00`;
  const date = new Date(value);
  return timeValue
    ? date.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })
    : date.toLocaleDateString([], { dateStyle: "medium" });
}

function formatWeekRangeLabel(weekStart) {
  const startDate = new Date(`${weekStart}T00:00:00`);
  const endDate = new Date(startDate);
  endDate.setDate(startDate.getDate() + 6);

  if (startDate.getFullYear() !== endDate.getFullYear()) {
    return `${startDate.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      year: "numeric",
    })} - ${endDate.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      year: "numeric",
    })}`;
  }

  return `${startDate.toLocaleDateString([], {
    month: "short",
    day: "numeric",
  })} - ${endDate.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
  })}`;
}

function todayIsoDate() {
  return new Date().toISOString().slice(0, 10);
}

function currentMondayIsoDate() {
  const today = new Date();
  const monday = new Date(today);
  const dayOffset = (today.getDay() + 6) % 7;
  monday.setDate(today.getDate() - dayOffset);
  return monday.toISOString().slice(0, 10);
}

function isMondayDate(value) {
  if (!value) return false;
  const date = new Date(`${value}T00:00:00`);
  return !Number.isNaN(date.getTime()) && date.getDay() === 1;
}

function formatTimestamp(item) {
  const timestamp = item.created_at || item.submitted_at || item.updated_at;
  if (!timestamp) return "Unknown date";
  return new Date(timestamp * 1000).toLocaleString();
}

function isSurveyPath() {
  return window.location.pathname.replace(/\/+$/, "") === "/survey";
}

function ArrowRightIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M13.3 5.3 20 12l-6.7 6.7-1.4-1.4 4.3-4.3H4v-2h12.2l-4.3-4.3 1.4-1.4Z" />
    </svg>
  );
}

function ReplyIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10 5 3 12l7 7v-4h4.5c2.6 0 4.8 1.5 5.9 3.7.4-1 .6-2 .6-3.1 0-4.2-3.4-7.6-7.6-7.6H10V5Z" />
    </svg>
  );
}

function CommentIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 5h14v10H8.8L5 18.8V5Zm2 2v7l1-1h9V7H7Z" />
    </svg>
  );
}

export default function App() {
  const [bootstrapping, setBootstrapping] = createSignal(true);
  const [session, setSession] = createSignal(null);
  const [sessionKind, setSessionKind] = createSignal("none");
  const [error, setError] = createSignal("");
  const [notice, setNotice] = createSignal("");
  const [dismissedAccountWarning, setDismissedAccountWarning] =
    createSignal(false);
  const [currentView, setCurrentView] = createSignal("home");
  const [signInForm, setSignInForm] = createSignal(emptySignIn);
  const [signUpForm, setSignUpForm] = createSignal(emptySignUp);
  const [posts, setPosts] = createSignal([]);
  const [feedPage, setFeedPage] = createSignal(1);
  const [feedHasMore, setFeedHasMore] = createSignal(false);
  const [feedLoading, setFeedLoading] = createSignal(false);
  const [thread, setThread] = createSignal(null);
  const [notifications, setNotifications] = createSignal([]);
  const [unreadCount, setUnreadCount] = createSignal(0);
  const [showNotifications, setShowNotifications] = createSignal(false);
  const [showMobileAccountMenu, setShowMobileAccountMenu] = createSignal(false);
  const [highlightedReplyId, setHighlightedReplyId] = createSignal(null);
  const [highlightedAdminTarget, setHighlightedAdminTarget] =
    createSignal(null);
  const [adminData, setAdminData] = createSignal({
    pending_users: [],
    pending_anonymous_posts: [],
    users: [],
    events: [],
    study_topics: [],
    survey_responses: [],
    survey_response_page: 1,
    survey_response_has_more: false,
  });
  const [postForm, setPostForm] = createSignal({ body: "", anonymous: false });
  const [showPostModal, setShowPostModal] = createSignal(false);
  const [replyBody, setReplyBody] = createSignal("");
  const [focusReplyOnOpen, setFocusReplyOnOpen] = createSignal(false);
  const [landingData, setLandingData] = createSignal({
    upcoming_events: [],
    current_study_topic: null,
    has_upcoming_study_topics: false,
  });
  const [upcomingStudyTopics, setUpcomingStudyTopics] = createSignal([]);
  const [showUpcomingStudyTopics, setShowUpcomingStudyTopics] =
    createSignal(false);
  const [eventForm, setEventForm] = createSignal(emptyEventForm);
  const [studyTopicForm, setStudyTopicForm] = createSignal(emptyStudyTopicForm);
  const [adminModal, setAdminModal] = createSignal(null);
  const [showSurveyModal, setShowSurveyModal] = createSignal(false);
  const [surveyForm, setSurveyForm] = createSignal(emptySurveyForm);
  const [surveySubmitting, setSurveySubmitting] = createSignal(false);
  const [surveyResponsesLoading, setSurveyResponsesLoading] =
    createSignal(false);
  const [changePasswordForm, setChangePasswordForm] = createSignal({
    current_password: "",
    new_password: "",
    confirm_password: "",
  });
  let notificationToggleInFlight = false;
  let notificationAnchor;
  let mobileAccountMenuAnchor;
  let feedSentinel;
  let replyTextarea;
  let highlightTimer;

  const canReadFeed = createMemo(() => {
    const current = session();
    if (!current) return false;
    return (
      current.account_status === "active" ||
      current.account_status === "suspended"
    );
  });

  const canCreateNamedSurveyPost = createMemo(
    () => sessionKind() === "full" && session()?.account_status === "active",
  );

  createEffect(() => {
    session()?.account_status;
    setDismissedAccountWarning(false);
  });

  const adminUsers = createMemo(() => {
    const users = new Map();
    for (const user of adminData().pending_users) {
      users.set(user.id, { account_status: "pending_approval", ...user });
    }
    for (const user of adminData().users) {
      users.set(user.id, { ...users.get(user.id), ...user });
    }
    return Array.from(users.values());
  });

  async function bootstrap() {
    setBootstrapping(true);
    try {
      await api.ensureCsrf();
      await refreshLanding();
      const response = await api.session();
      setSession(response.user);
      setSessionKind(response.session_kind);
      await refreshData(response.user, response.session_kind);
    } catch {
      await refreshLanding();
      setSession(null);
      setSessionKind("none");
    } finally {
      setBootstrapping(false);
    }
  }

  async function refreshLanding() {
    const response = await api.landing();
    setLandingData({
      upcoming_events: response.upcoming_events || [],
      current_study_topic: response.current_study_topic || null,
      has_upcoming_study_topics: !!response.has_upcoming_study_topics,
    });
  }

  async function refreshData(
    currentUser = session(),
    currentKind = sessionKind(),
  ) {
    await refreshLanding();
    if (!currentUser || currentKind !== "full") return;
    if (
      currentUser.account_status === "active" ||
      currentUser.account_status === "suspended"
    ) {
      await loadFeedPage(1);
    } else {
      setPosts([]);
      setThread(null);
      setFeedHasMore(false);
      setFeedPage(1);
    }

    await refreshNotifications(currentUser);
    if (currentUser.is_admin) await refreshAdminData();
  }

  async function refreshAdminData() {
    const [pending, anonymousPosts, users, events, topics, surveyResponses] =
      await Promise.all([
        api.listPending(),
        api.listPendingAnonymousPosts(),
        api.listUsers(),
        api.listEvents(),
        api.listStudyTopics(),
        api.listSurveyResponses(1, SURVEY_RESPONSE_PAGE_SIZE),
      ]);
    const nextData = {
      pending_users: pending.pending_users || [],
      pending_anonymous_posts: anonymousPosts.posts || [],
      users: users.users || [],
      events: events.events || [],
      study_topics: topics.topics || [],
      survey_responses: surveyResponses.responses || [],
      survey_response_page: surveyResponses.page || 1,
      survey_response_has_more: !!surveyResponses.has_more,
    };
    setAdminData(nextData);
    return nextData;
  }

  async function loadMoreSurveyResponses() {
    if (surveyResponsesLoading() || !adminData().survey_response_has_more) {
      return;
    }
    setSurveyResponsesLoading(true);
    try {
      const nextPage = adminData().survey_response_page + 1;
      const surveyResponses = await api.listSurveyResponses(
        nextPage,
        SURVEY_RESPONSE_PAGE_SIZE,
      );
      setAdminData((current) => ({
        ...current,
        survey_responses: [
          ...current.survey_responses,
          ...(surveyResponses.responses || []),
        ],
        survey_response_page: surveyResponses.page || nextPage,
        survey_response_has_more: !!surveyResponses.has_more,
      }));
    } catch (err) {
      setError(err.message);
    } finally {
      setSurveyResponsesLoading(false);
    }
  }

  async function refreshNotifications(currentUser = session()) {
    if (!currentUser) return [];
    const standard = await api.listNotifications();
    let merged = [...(standard.notifications || [])].map((item) => ({
      ...item,
      source: "standard",
    }));
    let unread = standard.unread_count || 0;

    merged.sort((left, right) => right.created_at - left.created_at);
    setNotifications(merged);
    setUnreadCount(unread);
    return merged;
  }

  async function submitSignIn(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    const values = signInForm();
    try {
      const response = await api.signIn({
        email: values.email,
        password: values.password,
      });
      setSession(response.user);
      setSessionKind(response.session_kind);
      setCurrentView("home");
      await refreshData(response.user, response.session_kind);
    } catch (err) {
      setError(err.message);
    } finally {
      setSignInForm({ ...signInForm(), password: "" });
    }
  }

  async function submitSignUp(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    const values = signUpForm();
    try {
      const response = await api.signUp({
        first_name: values.first_name,
        last_name: values.last_name,
        email: values.email,
        password: values.password,
      });
      setSession(response.user);
      setSessionKind(response.session_kind);
      setNotice("Your account was created");
      setCurrentView("home");
      await refreshData(response.user, response.session_kind);
    } catch (err) {
      setError(err.message);
    } finally {
      setSignUpForm({ ...signUpForm(), password: "" });
    }
  }

  async function submitChangePassword(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    try {
      const existingPassword = changePasswordForm().current_password;
      const newPassword = changePasswordForm().new_password;
      const confirmPassword = changePasswordForm().confirm_password;
      if (newPassword.length < 12) {
        throw new Error("New password must be at least 12 characters");
      }
      if (newPassword !== confirmPassword) {
        throw new Error("New password and confirmation do not match");
      }
      const response = await api.changePassword({
        current_password: existingPassword,
        new_password: newPassword,
      });
      setSession(response.user);
      setSessionKind(response.session_kind);
      setChangePasswordForm({
        current_password: "",
        new_password: "",
        confirm_password: "",
      });
      setNotice("Password changed");
      await refreshData(response.user, response.session_kind);
    } catch (err) {
      setError(err.message);
    }
  }

  async function signOut() {
    await api.logout();
    setSession(null);
    setSessionKind("none");
    setPosts([]);
    setThread(null);
    setNotifications([]);
    setShowNotifications(false);
    setShowMobileAccountMenu(false);
    setShowPostModal(false);
    setAdminModal(null);
    setHighlightedReplyId(null);
    setHighlightedAdminTarget(null);
    setCurrentView("home");
    setFeedHasMore(false);
    setFeedPage(1);
    setAdminData({
      pending_users: [],
      pending_anonymous_posts: [],
      users: [],
      events: [],
      study_topics: [],
      survey_responses: [],
      survey_response_page: 1,
      survey_response_has_more: false,
    });
    await refreshLanding();
  }

  async function loadFeedPage(page = 1, append = false) {
    if (!canReadFeed() || feedLoading()) return;
    setFeedLoading(true);
    try {
      const feed = await api.listPosts(page, FEED_PAGE_SIZE);
      const nextPosts = feed.posts || [];
      setPosts((current) => (append ? [...current, ...nextPosts] : nextPosts));
      setFeedPage(feed.page || page);
      setFeedHasMore(nextPosts.length >= (feed.page_size || FEED_PAGE_SIZE));
    } catch (err) {
      setError(err.message);
    } finally {
      setFeedLoading(false);
    }
  }

  async function loadThread(postId) {
    try {
      setThread(await api.getThread(postId));
      return true;
    } catch (err) {
      setError(err.message);
      return false;
    }
  }

  async function openThread(postId, options = {}) {
    setError("");
    setShowNotifications(false);
    setShowMobileAccountMenu(false);
    setFocusReplyOnOpen(!!options.focusReply);
    return loadThread(postId);
  }

  function closeThread() {
    setThread(null);
    setReplyBody("");
    setFocusReplyOnOpen(false);
    setHighlightedReplyId(null);
  }

  function scrollToTarget(target) {
    window.setTimeout(() => {
      const element = document.querySelector(
        `[data-scroll-target="${selectorValue(target)}"]`,
      );
      element?.scrollIntoView({ block: "center", behavior: "smooth" });
    }, 0);
  }

  function highlightTarget(kind, id) {
    if (highlightTimer) window.clearTimeout(highlightTimer);
    setHighlightedReplyId(kind === "reply" ? id : null);
    setHighlightedAdminTarget(kind === "admin" ? id : null);
    if (id) scrollToTarget(`${kind}:${id}`);
    highlightTimer = window.setTimeout(() => {
      setHighlightedReplyId(null);
      setHighlightedAdminTarget(null);
      highlightTimer = null;
    }, HIGHLIGHT_CLEAR_MS);
  }

  async function openNotificationThread(item) {
    if (!item.post_id) {
      setNotice("This notification does not point to a post");
      return;
    }
    setCurrentView("home");
    setShowNotifications(false);
    setThread(null);
    if (await loadThread(item.post_id)) {
      highlightTarget("reply", item.reply_id || item.post_id);
    }
  }

  async function openAdminNotification(item) {
    if (!session()?.is_admin) {
      setNotice("Only admins can open that approval");
      return;
    }
    setCurrentView("admin");
    setThread(null);
    setShowNotifications(false);
    const admin = await refreshAdminData();
    if (item.kind === "pending_user") {
      const targetId = item.actor_user_id;
      if (admin.pending_users.some((user) => user.id === targetId)) {
        highlightTarget("admin", `user:${targetId}`);
      } else {
        setNotice("That user approval is no longer pending");
      }
      return;
    }
    if (item.kind === "pending_anonymous_post") {
      const targetId = item.post_id;
      if (
        targetId &&
        admin.pending_anonymous_posts.some((post) => post.id === targetId)
      ) {
        highlightTarget("admin", `anonymous-post:${targetId}`);
      } else {
        setNotice("That anonymous post approval is no longer pending");
      }
      return;
    }
    setNotice("That approval is no longer pending");
  }

  async function openNotification(item) {
    setError("");
    setNotice("");
    try {
      if (item.kind === "reply") {
        await openNotificationThread(item);
      } else if (item.kind === "pending_user") {
        await openAdminNotification(item);
      } else if (item.kind === "pending_anonymous_post") {
        await openAdminNotification(item);
      } else if (item.kind === "user_approved") {
        setCurrentView("home");
        setShowNotifications(false);
      } else {
        setNotice("This notification does not have a destination");
      }
    } catch (err) {
      setError(err.message);
    }
  }

  async function submitPost(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    try {
      const wasAnonymous = postForm().anonymous;
      const created = await api.createPost(
        { body: postForm().body },
        wasAnonymous,
      );
      setPostForm({ body: "", anonymous: false });
      setShowPostModal(false);
      await refreshData();
      setNotice(
        wasAnonymous
          ? created?.approval_status === "approved"
            ? "Anonymous post published. It cannot be edited or deleted, and replies will not send notifications."
            : "Anonymous post submitted for approval"
          : "Post published",
      );
    } catch (err) {
      if (err.status === 429) {
        setError(POST_RATE_LIMIT_MESSAGE);
        return;
      }
      setError(err.message);
    }
  }

  function closeSurveyModal() {
    setShowSurveyModal(false);
    if (isSurveyPath()) {
      window.history.replaceState(null, "", `/${window.location.search}`);
    }
  }

  async function submitSurvey(event) {
    event.preventDefault();
    if (surveySubmitting()) return;
    setError("");
    setNotice("");
    setSurveySubmitting(true);
    try {
      const values = surveyForm();
      const heart = values.heart.trim();
      const foodSuggestions = values.food_suggestions.trim();
      const dietaryRestrictions = values.dietary_restrictions.trim();
      const submitPostAnonymously =
        !canCreateNamedSurveyPost() || values.anonymous;

      if (heart) {
        await api.createPost({ body: heart }, submitPostAnonymously);
      }

      if (foodSuggestions || dietaryRestrictions) {
        try {
          await api.createSurveyResponse({
            food_suggestions: foodSuggestions || null,
            dietary_restrictions: dietaryRestrictions || null,
          });
        } catch (err) {
          if (err.status !== 429) throw err;
        }
      }

      setSurveyForm({
        ...emptySurveyForm,
        anonymous: !canCreateNamedSurveyPost(),
      });
      closeSurveyModal();
      await refreshData();
      setNotice("Survey submitted");
    } catch (err) {
      if (err.status === 429) {
        setError(POST_RATE_LIMIT_MESSAGE);
        return;
      }
      setError(err.message);
    } finally {
      setSurveySubmitting(false);
    }
  }

  async function submitReply(event) {
    event.preventDefault();
    if (!thread()) return;
    setError("");
    try {
      await api.createReply(thread().post.id, { body: replyBody() });
      const postId = thread().post.id;
      setReplyBody("");
      setPosts((current) =>
        current.map((post) =>
          post.id === postId
            ? { ...post, reply_count: (post.reply_count || 0) + 1 }
            : post,
        ),
      );
      await loadThread(postId);
      await refreshNotifications();
    } catch (err) {
      setError(err.message);
    }
  }

  async function markNotificationsRead(ids) {
    if (ids.length) {
      await api.markNotificationsRead(ids);
    }
  }

  async function clearNotifications() {
    try {
      await api.clearNotifications();
      setNotifications([]);
      setUnreadCount(0);
    } catch (err) {
      setError(err.message);
    }
  }

  function openManageAccount() {
    if (currentView() === "account") {
      returnToLanding();
      return;
    }
    setCurrentView("account");
    setThread(null);
    setShowNotifications(false);
    setShowMobileAccountMenu(false);
  }

  function closeManageAccount() {
    setCurrentView("home");
    setShowNotifications(false);
    setShowMobileAccountMenu(false);
  }

  function returnToLanding() {
    setCurrentView("home");
    closeThread();
    setShowNotifications(false);
    setShowMobileAccountMenu(false);
  }

  async function openNotifications() {
    const next = !showNotifications();
    setShowNotifications(next);
    setShowMobileAccountMenu(false);
    if (!next || notificationToggleInFlight) return;
    notificationToggleInFlight = true;
    try {
      const freshNotifications = await refreshNotifications();
      await markNotificationsRead(
        freshNotifications
          .filter((item) => item.source === "standard" && !item.read_at)
          .map((item) => item.id),
      );
      await refreshNotifications();
    } catch (err) {
      setError(err.message);
    } finally {
      notificationToggleInFlight = false;
    }
  }

  async function deleteOwnAccount() {
    const currentPassword = window.prompt(
      "Enter your current password to permanently delete your account. Anonymous posts you created will not be deleted.",
    );
    if (!currentPassword) return;
    try {
      await api.deleteOwnAccount({
        current_password: currentPassword,
      });
      await signOut();
    } catch (err) {
      setError(err.message);
    }
  }

  async function adminAction(action) {
    try {
      await action();
      await refreshData();
    } catch (err) {
      setError(err.message);
    }
  }

  function adminUserLabel(user) {
    const name = `${user.first_name || ""} ${user.last_name || ""}`.trim();
    return name || user.email || "this user";
  }

  function confirmAdminUserAction(user, action) {
    const label = adminUserLabel(user);
    const messages = {
      makeAdmin: `Make ${label} an admin?`,
      removeAdmin: `Remove admin access from ${label}?`,
      suspend: `Suspend ${label}? They will not be able to post or reply until reactivated.`,
      lock: `Lock ${label}? They will not be able to access the feed until reactivated.`,
      resetPassword: `Reset the password for ${label}? Their current password will stop working.`,
      delete: `Permanently delete ${label}? This cannot be undone.`,
    };
    return window.confirm(messages[action]);
  }

  function confirmPostDelete() {
    return window.confirm("Permanently delete this post? This cannot be undone.");
  }

  function adminEventLabel(item) {
    return item.name || "this event";
  }

  function confirmAdminEventDelete(item) {
    return window.confirm(
      `Permanently delete ${adminEventLabel(item)}? This cannot be undone.`,
    );
  }

  function adminStudyTopicLabel(item) {
    return item.name || "this study topic";
  }

  function confirmAdminStudyTopicDelete(item) {
    return window.confirm(
      `Permanently delete ${adminStudyTopicLabel(item)}? This cannot be undone.`,
    );
  }

  async function resetUserPassword(user) {
    if (!confirmAdminUserAction(user, "resetPassword")) return;
    try {
      const result = await api.resetPassword(user.id);
      window.alert(`Temporary password: ${result.temporary_password}`);
      await refreshData();
    } catch (err) {
      setError(err.message);
    }
  }

  async function toggleUpcomingStudyTopics() {
    const next = !showUpcomingStudyTopics();
    setShowUpcomingStudyTopics(next);
    if (next && upcomingStudyTopics().length === 0) {
      try {
        const response = await api.listUpcomingStudyTopics();
        setUpcomingStudyTopics(response.topics || []);
      } catch (err) {
        setError(err.message);
      }
    }
  }

  function beginEditEvent(event) {
    setEventForm({
      id: event.id,
      name: event.name,
      event_date: event.event_date,
      event_time: event.event_time || "",
      end_date: event.end_date || "",
      end_time: event.end_time || "",
      location: event.location || "",
      description: event.description || "",
    });
    setAdminModal("event");
  }

  function beginEditStudyTopic(topic) {
    setStudyTopicForm({
      id: topic.id,
      week_start: topic.week_start,
      name: topic.name,
      description: topic.description || "",
      hyperlink: topic.hyperlink || "",
    });
    setAdminModal("study-topic");
  }

  function beginAddEvent() {
    setEventForm({ ...emptyEventForm, event_date: todayIsoDate() });
    setAdminModal("event");
  }

  function beginAddStudyTopic() {
    setStudyTopicForm({
      ...emptyStudyTopicForm,
      week_start: currentMondayIsoDate(),
    });
    setAdminModal("study-topic");
  }

  function closeAdminModal() {
    setAdminModal(null);
    setEventForm(emptyEventForm);
    setStudyTopicForm(emptyStudyTopicForm);
  }

  async function submitEvent(event) {
    event.preventDefault();
    setError("");
    const payload = {
      name: eventForm().name,
      event_date: eventForm().event_date,
      event_time: eventForm().event_time || null,
      end_date: eventForm().end_date || null,
      end_time: eventForm().end_time || null,
      location: eventForm().location || null,
      description: eventForm().description || null,
    };
    await adminAction(async () => {
      if (eventForm().id) {
        await api.updateEvent(eventForm().id, payload);
      } else {
        await api.createEvent(payload);
      }
      setEventForm(emptyEventForm);
      setAdminModal(null);
      await refreshLanding();
    });
  }

  async function submitStudyTopic(event) {
    event.preventDefault();
    setError("");
    if (!isMondayDate(studyTopicForm().week_start)) {
      setError(STUDY_WEEK_START_MESSAGE);
      return;
    }
    const payload = {
      week_start: studyTopicForm().week_start,
      name: studyTopicForm().name,
      description: studyTopicForm().description || null,
      hyperlink: studyTopicForm().hyperlink || null,
    };
    await adminAction(async () => {
      if (studyTopicForm().id) {
        await api.updateStudyTopic(studyTopicForm().id, payload);
      } else {
        await api.createStudyTopic(payload);
      }
      setStudyTopicForm(emptyStudyTopicForm);
      setAdminModal(null);
      setUpcomingStudyTopics([]);
      await refreshLanding();
    });
  }

  function LandingSection(props) {
    return (
      <section
        classList={{
          "landing-panel": true,
          "with-feed-divider": !!props?.withFeedDivider,
        }}
      >
        <section class="landing-section study-section">
          <div class="section-title-row">
            <h2>Study this week</h2>
          </div>
          <Show
            when={landingData().current_study_topic}
            fallback={
              <p class="muted">No study topic has been posted for this week</p>
            }
          >
            {(topic) => (
              <a
                classList={{
                  "landing-list-item": true,
                  "study-topic-item": true,
                  "landing-link-item": !!topic().hyperlink,
                  "with-upcoming-study-topics":
                    showUpcomingStudyTopics() && upcomingStudyTopics().length > 0,
                }}
                href={topic().hyperlink || undefined}
                target={topic().hyperlink ? "_blank" : undefined}
                rel={topic().hyperlink ? "noreferrer" : undefined}
              >
                <div class="post-meta">
                  <span>
                    Week of {formatWeekRangeLabel(topic().week_start)}
                  </span>
                </div>
                <h3>{topic().name}</h3>
                <Show when={topic().description}>
                  <p>{topic().description}</p>
                </Show>
              </a>
            )}
          </Show>
          <Show when={showUpcomingStudyTopics()}>
            <div class="landing-list upcoming-study-list">
              <For each={upcomingStudyTopics()}>
                {(topic) => (
                  <a
                    classList={{
                      "landing-list-item": true,
                      "study-topic-item": true,
                      "landing-link-item": !!topic.hyperlink,
                    }}
                    href={topic.hyperlink || undefined}
                    target={topic.hyperlink ? "_blank" : undefined}
                    rel={topic.hyperlink ? "noreferrer" : undefined}
                  >
                    <div class="post-meta">
                      <span>
                        Week of {formatWeekRangeLabel(topic.week_start)}
                      </span>
                    </div>
                    <h3>{topic.name}</h3>
                    <Show when={topic.description}>
                      <p>{topic.description}</p>
                    </Show>
                  </a>
                )}
              </For>
              <Show when={upcomingStudyTopics().length === 0}>
                <p class="muted">No upcoming study topics have been posted</p>
              </Show>
            </div>
          </Show>
          <Show when={landingData().has_upcoming_study_topics}>
            <button
              class="upcoming-study-toggle"
              type="button"
              onClick={toggleUpcomingStudyTopics}
            >
              {showUpcomingStudyTopics()
                ? "- Hide upcoming weeks"
                : "+ Show upcoming weeks"}
            </button>
          </Show>
        </section>

        <section class="landing-section">
          <div class="section-title-row">
            <h2>Upcoming events</h2>
          </div>
          <Show
            when={landingData().upcoming_events.length > 0}
            fallback={
              <p class="muted">No upcoming events have been posted yet</p>
            }
          >
            <div class="landing-list landing-event-list">
              <For each={landingData().upcoming_events}>
                {(item) => (
                  <article class="landing-list-item landing-event-item">
                    <div class="post-meta">
                      <span>{formatEventDate(item)}</span>
                      <Show when={item.location}>
                        <span>{item.location}</span>
                      </Show>
                    </div>
                    <h3>{item.name}</h3>
                    <Show when={item.description}>
                      <p>{item.description}</p>
                    </Show>
                  </article>
                )}
              </For>
            </div>
          </Show>
        </section>
      </section>
    );
  }

  function SignInView() {
    return (
      <section class="panel auth-panel">
        <h2>Sign in</h2>
        <form class="stack-form narrow" onSubmit={submitSignIn}>
          <input
            type="email"
            placeholder="Email address"
            value={signInForm().email}
            onInput={(event) =>
              setSignInForm({
                ...signInForm(),
                email: event.currentTarget.value,
              })
            }
          />
          <input
            type="password"
            placeholder="Password"
            value={signInForm().password}
            onInput={(event) =>
              setSignInForm({
                ...signInForm(),
                password: event.currentTarget.value,
              })
            }
          />
          <button class="primary-button" type="submit">
            Sign in
          </button>
        </form>
      </section>
    );
  }

  function SignUpView() {
    return (
      <section class="panel auth-panel">
        <h2>Create account</h2>
        <form class="stack-form narrow" onSubmit={submitSignUp}>
          <input
            type="text"
            placeholder="First name"
            value={signUpForm().first_name}
            onInput={(event) =>
              setSignUpForm({
                ...signUpForm(),
                first_name: event.currentTarget.value,
              })
            }
          />
          <input
            type="text"
            placeholder="Last name"
            value={signUpForm().last_name}
            onInput={(event) =>
              setSignUpForm({
                ...signUpForm(),
                last_name: event.currentTarget.value,
              })
            }
          />
          <input
            type="email"
            placeholder="Email address"
            value={signUpForm().email}
            onInput={(event) =>
              setSignUpForm({
                ...signUpForm(),
                email: event.currentTarget.value,
              })
            }
          />
          <input
            type="password"
            placeholder="Password"
            value={signUpForm().password}
            onInput={(event) =>
              setSignUpForm({
                ...signUpForm(),
                password: event.currentTarget.value,
              })
            }
          />
          <button class="primary-button" type="submit">
            Create account
          </button>
        </form>
      </section>
    );
  }

  function NotificationPanel() {
    return (
      <div class="notifications-box">
        <div class="section-title-row">
          <h2>Notifications</h2>
          <button class="ghost-button" onClick={clearNotifications}>
            Clear notifications
          </button>
        </div>
        <div class="notification-list">
          <For each={notifications()}>
            {(item) => (
              <button
                type="button"
                classList={{ notification: true, unread: !item.read_at }}
                onClick={() => openNotification(item)}
              >
                <div>
                  <strong>
                    {item.kind === "pending_anonymous_post"
                      ? "Anonymous post"
                      : "Account"}
                  </strong>
                  <p>{item.message}</p>
                </div>
                <span>{new Date(item.created_at * 1000).toLocaleString()}</span>
              </button>
            )}
          </For>
          <Show when={notifications().length === 0}>
            <p class="muted">No notifications</p>
          </Show>
        </div>
      </div>
    );
  }

  function DismissibleMessage(props) {
    return (
      <div class={`message ${props.variant || "warning"}`}>
        <div class="message-body">{props.children}</div>
        <button
          class="message-close"
          type="button"
          aria-label="Dismiss message"
          onClick={props.onDismiss}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m6.4 5 5.6 5.6L17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4 6.4 5Z" />
          </svg>
        </button>
      </div>
    );
  }

  function ManageAccountView() {
    return (
      <section class="account-stack">
        <section class="panel">
          <div class="section-title-row">
            <div>
              <p class="eyebrow">Signed in as</p>
              <h2>
                {session()?.first_name} {session()?.last_name}
              </h2>
            </div>
            <div class="post-actions wrap">
              <button class="ghost-button danger" onClick={deleteOwnAccount}>
                Delete account
              </button>
            </div>
          </div>
          <Show
            when={
              session()?.account_status !== "active" &&
              !dismissedAccountWarning()
            }
          >
            <DismissibleMessage
              variant="warning"
              onDismiss={() => setDismissedAccountWarning(true)}
            >
              {statusCopy(session()?.account_status)}
            </DismissibleMessage>
          </Show>
        </section>

        <section class="panel">
          <h2>Change password</h2>
          <form class="stack-form narrow" onSubmit={submitChangePassword}>
            <input
              type="password"
              placeholder="Current password"
              value={changePasswordForm().current_password}
              onInput={(event) =>
                setChangePasswordForm({
                  ...changePasswordForm(),
                  current_password: event.currentTarget.value,
                })
              }
            />
            <input
              type="password"
              placeholder="New password"
              value={changePasswordForm().new_password}
              onInput={(event) =>
                setChangePasswordForm({
                  ...changePasswordForm(),
                  new_password: event.currentTarget.value,
                })
              }
            />
            <input
              type="password"
              placeholder="Confirm new password"
              value={changePasswordForm().confirm_password}
              onInput={(event) =>
                setChangePasswordForm({
                  ...changePasswordForm(),
                  confirm_password: event.currentTarget.value,
                })
              }
            />
            <button class="primary-button" type="submit">
              Change password
            </button>
          </form>
        </section>
      </section>
    );
  }

  function FeedSection() {
    return (
      <Show
        when={
          session()?.account_status !== "locked" &&
          session()?.account_status !== "pending_approval"
        }
      >
        <section class="landing-section feed-panel">
          <div class="section-title-row">
            <h2>
              What is on your heart or mind that you'd like to share with the
              quorum?
            </h2>
            <div class="post-actions">
              <Show when={session()?.account_status === "active"}>
                <button
                  class="primary-button"
                  onClick={() => setShowPostModal(true)}
                >
                  Post
                </button>
              </Show>
            </div>
          </div>
          <div class="feed-list">
            <For each={posts()}>
              {(post) => (
                <article class="post-card">
                  <div class="post-meta">
                    <div>
                      <span
                        classList={{
                          "post-author": true,
                          "anonymous-author": post.is_anonymous,
                        }}
                      >
                        {post.is_anonymous ? "Anonymous" : post.author_name}
                      </span>
                    </div>
                    <time
                      dateTime={new Date(post.created_at * 1000).toISOString()}
                    >
                      {new Date(post.created_at * 1000).toLocaleString()}
                    </time>
                  </div>
                  <p class="post-body">{post.body}</p>
                  <div class="post-actions feed-post-actions">
                    <div class="view-thread-group">
                      <button
                        class="ghost-button icon-label-button"
                        onClick={() => openThread(post.id)}
                      >
                        <span>View thread</span>
                        <ArrowRightIcon />
                      </button>
                      <Show when={(post.reply_count || 0) > 0}>
                        <span class="reply-count" aria-label={`${post.reply_count} replies`}>
                          <CommentIcon />
                          <span>{post.reply_count}</span>
                        </span>
                      </Show>
                    </div>
                    <div class="post-secondary-actions">
                      <Show when={session()?.account_status === "active"}>
                        <button
                          class="ghost-button icon-label-button"
                          onClick={() =>
                            openThread(post.id, { focusReply: true })
                          }
                        >
                          <ReplyIcon />
                          <span>Reply</span>
                        </button>
                      </Show>
                      <Show
                        when={
                          !post.is_anonymous &&
                          post.author_user_id === session()?.id
                        }
                      >
                        <button
                          class="ghost-button danger"
                          onClick={() =>
                            confirmPostDelete() &&
                            adminAction(() => api.deletePost(post.id))
                          }
                        >
                          Delete
                        </button>
                      </Show>
                    </div>
                  </div>
                </article>
              )}
            </For>
            <Show when={posts().length === 0 && !feedLoading()}>
              <p class="muted">No posts yet</p>
            </Show>
          </div>
          <div class="feed-sentinel" ref={feedSentinel}>
            <Show when={feedLoading()}>
              <span class="muted">Loading posts</span>
            </Show>
          </div>
        </section>
      </Show>
    );
  }

  function ThreadModal() {
    return (
      <Show when={thread()}>
        <div class="modal-backdrop thread-backdrop" role="presentation">
          <section
            class="modal-panel thread-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="thread-modal-title"
          >
            <div class="section-title-row thread-modal-header">
              <h2 id="thread-modal-title">Thread</h2>
              <button class="ghost-button" type="button" onClick={closeThread}>
                Close
              </button>
            </div>

            <div class="thread-scroll">
              <article
                classList={{
                  "thread-original": true,
                  "notification-target":
                    highlightedReplyId() === thread().post.id,
                }}
                data-scroll-target={`reply:${thread().post.id}`}
              >
                <div class="thread-label">Original post</div>
                <div class="post-meta">
                  <div>
                    <span
                      classList={{
                        "post-author": true,
                        "anonymous-author": thread().post.is_anonymous,
                      }}
                    >
                      {thread().post.is_anonymous
                        ? "Anonymous"
                        : thread().post.author_name}
                    </span>
                  </div>
                  <time
                    dateTime={new Date(
                      thread().post.created_at * 1000,
                    ).toISOString()}
                  >
                    {new Date(thread().post.created_at * 1000).toLocaleString()}
                  </time>
                </div>
                <p class="post-body">{thread().post.body}</p>
              </article>

              <div class="thread-replies" aria-label="Replies">
                <Show
                  when={thread().replies.length > 0}
                  fallback={<p class="muted empty-thread">No replies yet</p>}
                >
                  <For each={thread().replies}>
                    {(reply) => (
                      <article
                        classList={{
                          "reply-card": true,
                          "thread-reply": true,
                          "notification-target":
                            highlightedReplyId() === reply.id,
                        }}
                        data-scroll-target={`reply:${reply.id}`}
                      >
                        <div class="reply-node" aria-hidden="true"></div>
                        <div class="reply-content">
                          <div class="post-meta">
                            <span class="post-author">{reply.author_name}</span>
                            <time
                              dateTime={new Date(
                                reply.created_at * 1000,
                              ).toISOString()}
                            >
                              {new Date(reply.created_at * 1000).toLocaleString()}
                            </time>
                          </div>
                          <p class="post-body">{reply.body}</p>
                          <Show
                            when={
                              reply.author_user_id === session()?.id ||
                              session()?.is_admin
                            }
                          >
                            <button
                              class="ghost-button danger"
                              type="button"
                              onClick={() =>
                                adminAction(() =>
                                  api
                                    .deleteReply(reply.id)
                                    .then(() => loadThread(thread().post.id)),
                                )
                              }
                            >
                              Delete
                            </button>
                          </Show>
                        </div>
                      </article>
                    )}
                  </For>
                </Show>
              </div>
            </div>

            <Show when={session()?.account_status === "active"}>
              <form class="stack-form thread-reply-form" onSubmit={submitReply}>
                <textarea
                  ref={replyTextarea}
                  rows="3"
                  placeholder="Write a reply"
                  value={replyBody()}
                  onInput={(event) => setReplyBody(event.currentTarget.value)}
                />
                <div class="post-actions wrap thread-form-actions">
                  <button class="primary-button icon-label-button" type="submit">
                    <ReplyIcon />
                    <span>Reply</span>
                  </button>
                </div>
              </form>
            </Show>
          </section>
        </div>
      </Show>
    );
  }

  function AccountStatusWarning() {
    return (
      <Show
        when={
          !dismissedAccountWarning() &&
          (session()?.account_status === "suspended" ||
            session()?.account_status === "locked")
        }
      >
        <section class="warning-card dismissible-warning">
          <Show
            when={session()?.account_status === "suspended"}
            fallback={
              <p>
                Your account has been locked. A member of the EQ presidency can
                unlock your account
              </p>
            }
          >
            <p>
              Your account has been suspended from posting or replying. A member
              of the EQ presidency can unsuspend your account
            </p>
          </Show>
          <button
            class="message-close"
            type="button"
            aria-label="Dismiss warning"
            onClick={() => setDismissedAccountWarning(true)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="m6.4 5 5.6 5.6L17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4 6.4 5Z" />
            </svg>
          </button>
        </section>
      </Show>
    );
  }

  function PostModal() {
    return (
      <Show when={showPostModal()}>
        <div class="modal-backdrop" role="presentation">
          <section
            class="modal-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="post-modal-title"
          >
            <div class="section-title-row">
              <h2 id="post-modal-title">Create post</h2>
              <button
                class="ghost-button"
                onClick={() => setShowPostModal(false)}
              >
                Close
              </button>
            </div>
            <form class="stack-form" onSubmit={submitPost}>
              <textarea
                rows="5"
                placeholder="Share a challenge you're going through, a question you have, or a spiritual thought"
                value={postForm().body}
                onInput={(event) =>
                  setPostForm({
                    ...postForm(),
                    body: event.currentTarget.value,
                  })
                }
              />
              <label class="toggle-row">
                <input
                  type="checkbox"
                  checked={postForm().anonymous}
                  onChange={(event) =>
                    setPostForm({
                      ...postForm(),
                      anonymous: event.currentTarget.checked,
                    })
                  }
                />
                <span>Post anonymously</span>
              </label>
              <Show when={postForm().anonymous}>
                <p class="muted">
                  Anonymous posts cannot be edited or deleted and you will not
                  receive notifications when someone replies
                </p>
              </Show>
              <button class="primary-button" type="submit">
                Publish
              </button>
            </form>
          </section>
        </div>
      </Show>
    );
  }

  function EventModal() {
    return (
      <Show when={adminModal() === "event"}>
        <div class="modal-backdrop" role="presentation">
          <section
            class="modal-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="event-modal-title"
          >
            <div class="section-title-row">
              <h2 id="event-modal-title">
                {eventForm().id ? "Edit event" : "Add event"}
              </h2>
              <button
                class="ghost-button"
                type="button"
                onClick={closeAdminModal}
              >
                Close
              </button>
            </div>
            <form class="stack-form" onSubmit={submitEvent}>
              <label class="field-group">
                <span>Event name (required)</span>
                <input
                  type="text"
                  value={eventForm().name}
                  required
                  onInput={(event) =>
                    setEventForm({
                      ...eventForm(),
                      name: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <div class="split-inputs">
                <label class="field-group">
                  <span>Start date (required)</span>
                  <input
                    type="date"
                    value={eventForm().event_date}
                    required
                    onInput={(event) =>
                      setEventForm({
                        ...eventForm(),
                        event_date: event.currentTarget.value,
                      })
                    }
                  />
                </label>
                <label class="field-group">
                  <span>Start time (optional)</span>
                  <input
                    type="time"
                    value={eventForm().event_time}
                    onInput={(event) =>
                      setEventForm({
                        ...eventForm(),
                        event_time: event.currentTarget.value,
                      })
                    }
                  />
                </label>
              </div>
              <div class="split-inputs">
                <label class="field-group">
                  <span>End date (optional)</span>
                  <input
                    type="date"
                    value={eventForm().end_date}
                    onInput={(event) =>
                      setEventForm({
                        ...eventForm(),
                        end_date: event.currentTarget.value,
                      })
                    }
                  />
                </label>
                <label class="field-group">
                  <span>End time (optional)</span>
                  <input
                    type="time"
                    value={eventForm().end_time}
                    onInput={(event) =>
                      setEventForm({
                        ...eventForm(),
                        end_time: event.currentTarget.value,
                      })
                    }
                  />
                </label>
              </div>
              <label class="field-group">
                <span>Location (optional)</span>
                <input
                  type="text"
                  value={eventForm().location}
                  onInput={(event) =>
                    setEventForm({
                      ...eventForm(),
                      location: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <label class="field-group">
                <span>Description (optional)</span>
                <textarea
                  rows="3"
                  value={eventForm().description}
                  onInput={(event) =>
                    setEventForm({
                      ...eventForm(),
                      description: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <div class="post-actions">
                <button class="primary-button" type="submit">
                  {eventForm().id ? "Save event" : "Create event"}
                </button>
                <button
                  class="ghost-button"
                  type="button"
                  onClick={closeAdminModal}
                >
                  Cancel
                </button>
              </div>
            </form>
          </section>
        </div>
      </Show>
    );
  }

  function StudyTopicModal() {
    return (
      <Show when={adminModal() === "study-topic"}>
        <div class="modal-backdrop" role="presentation">
          <section
            class="modal-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="study-topic-modal-title"
          >
            <div class="section-title-row">
              <h2 id="study-topic-modal-title">
                {studyTopicForm().id ? "Edit study topic" : "Add study topic"}
              </h2>
              <button
                class="ghost-button"
                type="button"
                onClick={closeAdminModal}
              >
                Close
              </button>
            </div>
            <form class="stack-form" onSubmit={submitStudyTopic}>
              <label class="field-group">
                <span>Week start (required)</span>
                <input
                  type="date"
                  min="1970-01-05"
                  step="7"
                  value={studyTopicForm().week_start}
                  required
                  onInput={(event) =>
                    setStudyTopicForm({
                      ...studyTopicForm(),
                      week_start: event.currentTarget.value,
                    })
                  }
                />
                <span class="field-note">
                  Choose the Monday that starts the study week.
                </span>
              </label>
              <label class="field-group">
                <span>Topic name (required)</span>
                <input
                  type="text"
                  value={studyTopicForm().name}
                  required
                  onInput={(event) =>
                    setStudyTopicForm({
                      ...studyTopicForm(),
                      name: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <label class="field-group">
                <span>Hyperlink (optional)</span>
                <input
                  type="url"
                  value={studyTopicForm().hyperlink}
                  onInput={(event) =>
                    setStudyTopicForm({
                      ...studyTopicForm(),
                      hyperlink: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <label class="field-group">
                <span>Description (optional)</span>
                <textarea
                  rows="3"
                  value={studyTopicForm().description}
                  onInput={(event) =>
                    setStudyTopicForm({
                      ...studyTopicForm(),
                      description: event.currentTarget.value,
                    })
                  }
                />
              </label>
              <div class="post-actions">
                <button class="primary-button" type="submit">
                  {studyTopicForm().id ? "Save topic" : "Create topic"}
                </button>
                <button
                  class="ghost-button"
                  type="button"
                  onClick={closeAdminModal}
                >
                  Cancel
                </button>
              </div>
            </form>
          </section>
        </div>
      </Show>
    );
  }

  function SurveyModal() {
    return (
      <Show when={showSurveyModal()}>
        <div class="modal-backdrop survey-backdrop" role="presentation">
          <section
            class="modal-panel survey-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="survey-modal-title"
          >
            <div class="section-title-row">
              <h2 id="survey-modal-title">Quorum Survey</h2>
              <button
                class="ghost-button"
                type="button"
                onClick={closeSurveyModal}
              >
                Close
              </button>
            </div>
            <p class="muted">All fields are optional</p>
            <form class="stack-form survey-form" onSubmit={submitSurvey}>
              <label class="field-group">
                <span>
                  What is on your heart or mind that you'd like to share with
                  the quorum? Share a challenge you're going through, a question
                  you have, or a spiritual thought.
                </span>
                <Show
                  when={canCreateNamedSurveyPost()}
                  fallback={
                    <p class="muted survey-note">
                      This response will be shared anonymously
                    </p>
                  }
                >
                  <label class="toggle-row survey-toggle">
                    <input
                      type="checkbox"
                      checked={surveyForm().anonymous}
                      onChange={(event) =>
                        setSurveyForm({
                          ...surveyForm(),
                          anonymous: event.currentTarget.checked,
                        })
                      }
                    />
                    <span>Keep my response anonymous</span>
                  </label>
                </Show>
                <textarea
                  rows="7"
                  value={surveyForm().heart}
                  onInput={(event) =>
                    setSurveyForm({
                      ...surveyForm(),
                      heart: event.currentTarget.value,
                    })
                  }
                />
              </label>

              <label class="field-group">
                <span>
                  Do you have suggestions for snacks/treats for Elders Quorum?
                </span>
                <textarea
                  rows="3"
                  value={surveyForm().food_suggestions}
                  onInput={(event) =>
                    setSurveyForm({
                      ...surveyForm(),
                      food_suggestions: event.currentTarget.value,
                    })
                  }
                />
              </label>

              <label class="field-group">
                <span>
                  Do you have any food allergies/dietary restrictions?
                </span>
                <textarea
                  rows="3"
                  value={surveyForm().dietary_restrictions}
                  onInput={(event) =>
                    setSurveyForm({
                      ...surveyForm(),
                      dietary_restrictions: event.currentTarget.value,
                    })
                  }
                />
              </label>

              <div class="post-actions wrap survey-actions">
                <button
                  class="primary-button"
                  type="submit"
                  disabled={surveySubmitting()}
                >
                  Submit survey
                </button>
                <button
                  class="ghost-button"
                  type="button"
                  onClick={closeSurveyModal}
                  disabled={surveySubmitting()}
                >
                  Cancel
                </button>
              </div>
            </form>
          </section>
        </div>
      </Show>
    );
  }

  function AdminDashboard() {
    return (
      <section class="admin-dashboard">
        <section class="panel admin-panel">
          <div class="section-title-row">
            <h2>User management</h2>
          </div>
          <div
            class="admin-table-wrap"
            role="region"
            tabIndex="0"
            aria-label="Scrollable user management table"
          >
            <table class="admin-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Email</th>
                  <th>Status</th>
                  <th>Role</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                <Show
                  when={adminUsers().length > 0}
                  fallback={
                    <tr>
                      <td class="empty-cell" colSpan="5">
                        No users
                      </td>
                    </tr>
                  }
                >
                  <For each={adminUsers()}>
                    {(user) => (
                      <tr
                        classList={{
                          "notification-target":
                            highlightedAdminTarget() === `user:${user.id}`,
                        }}
                        data-scroll-target={`admin:user:${user.id}`}
                      >
                        <td>
                          {user.first_name} {user.last_name}
                        </td>
                        <td>{user.email || "Unknown email"}</td>
                        <td>{user.account_status || "active"}</td>
                        <td>{user.is_admin ? "Admin" : "Member"}</td>
                        <td>
                          <div class="post-actions wrap">
                            <Show
                              when={
                                user.account_status === "pending_approval" ||
                                adminData().pending_users.some(
                                  (pending) => pending.id === user.id,
                                )
                              }
                            >
                              <button
                                class="ghost-button"
                                onClick={() =>
                                  adminAction(() => api.approveUser(user.id))
                                }
                              >
                                Approve
                              </button>
                            </Show>
                            <button
                              class="ghost-button"
                              onClick={() =>
                                confirmAdminUserAction(
                                  user,
                                  user.is_admin ? "removeAdmin" : "makeAdmin",
                                ) &&
                                adminAction(() =>
                                  api.setAdmin(user.id, !user.is_admin),
                                )
                              }
                            >
                              {user.is_admin ? "Remove admin" : "Make admin"}
                            </button>
                            <button
                              class="ghost-button"
                              onClick={() =>
                                adminAction(() =>
                                  api.setUserStatus(user.id, "active"),
                                )
                              }
                            >
                              Activate
                            </button>
                            <button
                              class="ghost-button"
                              onClick={() =>
                                confirmAdminUserAction(user, "suspend") &&
                                adminAction(() =>
                                  api.setUserStatus(user.id, "suspended"),
                                )
                              }
                            >
                              Suspend
                            </button>
                            <button
                              class="ghost-button"
                              onClick={() =>
                                confirmAdminUserAction(user, "lock") &&
                                adminAction(() =>
                                  api.setUserStatus(user.id, "locked"),
                                )
                              }
                            >
                              Lock
                            </button>
                            <button
                              class="ghost-button"
                              onClick={() => resetUserPassword(user)}
                            >
                              Reset password
                            </button>
                            <button
                              class="ghost-button danger"
                              onClick={() =>
                                confirmAdminUserAction(user, "delete") &&
                                adminAction(() => api.deleteUser(user.id))
                              }
                            >
                              Delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            </table>
          </div>
        </section>

        <section class="panel admin-panel">
          <div class="section-title-row">
            <h2>Anonymous post approvals</h2>
          </div>
          <div
            class="admin-table-wrap"
            role="region"
            tabIndex="0"
            aria-label="Scrollable anonymous post approvals table"
          >
            <table class="admin-table">
              <thead>
                <tr>
                  <th>Submitted</th>
                  <th class="wide-column">Post</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                <Show
                  when={adminData().pending_anonymous_posts.length > 0}
                  fallback={
                    <tr>
                      <td class="empty-cell" colSpan="3">
                        No anonymous posts awaiting approval
                      </td>
                    </tr>
                  }
                >
                  <For each={adminData().pending_anonymous_posts}>
                    {(post) => (
                      <tr
                        classList={{
                          "notification-target":
                            highlightedAdminTarget() ===
                            `anonymous-post:${post.id}`,
                        }}
                        data-scroll-target={`admin:anonymous-post:${post.id}`}
                      >
                        <td>{formatTimestamp(post)}</td>
                        <td class="wide-column preserve-lines">{post.body}</td>
                        <td>
                          <div class="post-actions wrap">
                            <button
                              class="ghost-button"
                              onClick={() =>
                                adminAction(() =>
                                  api.approveAnonymousPost(post.id),
                                )
                              }
                            >
                              Approve
                            </button>
                            <button
                              class="ghost-button danger"
                              onClick={() =>
                                adminAction(() =>
                                  api.deleteContent("post", post.id),
                                )
                              }
                            >
                              Delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            </table>
          </div>
        </section>

        <section class="panel admin-panel">
          <div class="section-title-row">
            <h2>Survey responses</h2>
          </div>
          <div
            class="admin-table-wrap"
            role="region"
            tabIndex="0"
            aria-label="Scrollable survey responses table"
          >
            <table class="admin-table survey-responses-table">
              <thead>
                <tr>
                  <th>Submitted</th>
                  <th class="wide-column">Food suggestions</th>
                  <th class="wide-column">Dietary restrictions</th>
                </tr>
              </thead>
              <tbody>
                <Show
                  when={adminData().survey_responses.length > 0}
                  fallback={
                    <tr>
                      <td class="empty-cell" colSpan="3">
                        No survey responses
                      </td>
                    </tr>
                  }
                >
                  <For each={adminData().survey_responses}>
                    {(response) => (
                      <tr>
                        <td>{formatTimestamp(response)}</td>
                        <td class="wide-column preserve-lines">
                          {response.food_suggestions || "No food suggestions"}
                        </td>
                        <td class="wide-column preserve-lines">
                          {response.dietary_restrictions ||
                            "No dietary restrictions"}
                        </td>
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            </table>
          </div>
          <Show when={adminData().survey_response_has_more}>
            <div class="admin-panel-actions">
              <button
                class="ghost-button"
                type="button"
                onClick={loadMoreSurveyResponses}
                disabled={surveyResponsesLoading()}
              >
                {surveyResponsesLoading() ? "Loading..." : "Show more"}
              </button>
            </div>
          </Show>
        </section>

        <section class="panel admin-panel">
          <div class="section-title-row">
            <h2>Upcoming events</h2>
            <button class="primary-button" onClick={beginAddEvent}>
              + Add event
            </button>
          </div>
          <div
            class="admin-table-wrap"
            role="region"
            tabIndex="0"
            aria-label="Scrollable upcoming events table"
          >
            <table class="admin-table">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Name</th>
                  <th>Location</th>
                  <th class="wide-column">Description</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                <Show
                  when={adminData().events.length > 0}
                  fallback={
                    <tr>
                      <td class="empty-cell" colSpan="5">
                        No upcoming events
                      </td>
                    </tr>
                  }
                >
                  <For each={adminData().events}>
                    {(item) => (
                      <tr>
                        <td>{formatEventDate(item)}</td>
                        <td>{item.name}</td>
                        <td>{item.location || "No location"}</td>
                        <td class="wide-column preserve-lines">
                          {item.description || "No description"}
                        </td>
                        <td>
                          <div class="post-actions wrap">
                            <button
                              class="ghost-button"
                              onClick={() => beginEditEvent(item)}
                            >
                              Edit
                            </button>
                            <button
                              class="ghost-button danger"
                              onClick={() =>
                                confirmAdminEventDelete(item) &&
                                adminAction(() => api.deleteEvent(item.id))
                              }
                            >
                              Delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            </table>
          </div>
        </section>

        <section class="panel admin-panel">
          <div class="section-title-row">
            <h2>Study topics</h2>
            <button class="primary-button" onClick={beginAddStudyTopic}>
              + Add study topic
            </button>
          </div>
          <div
            class="admin-table-wrap"
            role="region"
            tabIndex="0"
            aria-label="Scrollable study topics table"
          >
            <table class="admin-table">
              <thead>
                <tr>
                  <th>Week</th>
                  <th>Name</th>
                  <th class="wide-column">Description</th>
                  <th>Resource</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                <Show
                  when={adminData().study_topics.length > 0}
                  fallback={
                    <tr>
                      <td class="empty-cell" colSpan="5">
                        No study topics
                      </td>
                    </tr>
                  }
                >
                  <For each={adminData().study_topics}>
                    {(item) => (
                      <tr>
                        <td>Week of {formatWeekRangeLabel(item.week_start)}</td>
                        <td>{item.name}</td>
                        <td class="wide-column preserve-lines">
                          {item.description || "No description"}
                        </td>
                        <td>
                          <Show when={item.hyperlink} fallback="No resource">
                            <a
                              class="inline-link"
                              href={item.hyperlink}
                              target="_blank"
                              rel="noreferrer"
                            >
                              Open resource
                            </a>
                          </Show>
                        </td>
                        <td>
                          <div class="post-actions wrap">
                            <button
                              class="ghost-button"
                              onClick={() => beginEditStudyTopic(item)}
                            >
                              Edit
                            </button>
                            <button
                              class="ghost-button danger"
                              onClick={() =>
                                confirmAdminStudyTopicDelete(item) &&
                                adminAction(() => api.deleteStudyTopic(item.id))
                              }
                            >
                              Delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            </table>
          </div>
        </section>

        <EventModal />
        <StudyTopicModal />
      </section>
    );
  }

  createEffect(() => {
    if (
      currentView() === "account" ||
      currentView() === "admin" ||
      !canReadFeed() ||
      !feedHasMore() ||
      !feedSentinel
    )
      return;
    const observer = new IntersectionObserver(
      (entries) => {
        const [entry] = entries;
        if (entry.isIntersecting && !feedLoading() && feedHasMore()) {
          loadFeedPage(feedPage() + 1, true);
        }
      },
      { rootMargin: "240px" },
    );
    observer.observe(feedSentinel);
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    if (!canCreateNamedSurveyPost() && surveyForm().anonymous !== true) {
      setSurveyForm({ ...surveyForm(), anonymous: true });
    }
  });

  createEffect(() => {
    if (!thread() || !focusReplyOnOpen() || session()?.account_status !== "active") return;
    window.setTimeout(() => {
      replyTextarea?.focus();
      replyTextarea?.scrollIntoView({ block: "center", behavior: "smooth" });
      setFocusReplyOnOpen(false);
    }, 0);
  });

  onCleanup(() => {
    if (highlightTimer) window.clearTimeout(highlightTimer);
  });

  onMount(() => {
    const openSurveyFromUrl = () => {
      if (isSurveyPath()) {
        setCurrentView("home");
        setShowSurveyModal(true);
      }
    };
    const closeSurveyOnEscape = (event) => {
      if (event.key === "Escape" && showSurveyModal()) closeSurveyModal();
      if (event.key === "Escape" && thread()) closeThread();
    };
    const closeNotificationsOnOutsideClick = (event) => {
      if (
        !showNotifications() ||
        !notificationAnchor ||
        notificationAnchor.contains(event.target)
      ) {
        return;
      }
      setShowNotifications(false);
    };
    const closeMobileAccountMenuOnOutsideClick = (event) => {
      if (
        !showMobileAccountMenu() ||
        !mobileAccountMenuAnchor ||
        mobileAccountMenuAnchor.contains(event.target)
      ) {
        return;
      }
      setShowMobileAccountMenu(false);
    };
    openSurveyFromUrl();
    window.addEventListener("popstate", openSurveyFromUrl);
    window.addEventListener("keydown", closeSurveyOnEscape);
    document.addEventListener("pointerdown", closeNotificationsOnOutsideClick);
    document.addEventListener(
      "pointerdown",
      closeMobileAccountMenuOnOutsideClick,
    );
    onCleanup(() => {
      window.removeEventListener("popstate", openSurveyFromUrl);
      window.removeEventListener("keydown", closeSurveyOnEscape);
      document.removeEventListener(
        "pointerdown",
        closeNotificationsOnOutsideClick,
      );
      document.removeEventListener(
        "pointerdown",
        closeMobileAccountMenuOnOutsideClick,
      );
    });
    bootstrap();
  });

  return (
    <div class="page-shell">
      <Show
        when={!bootstrapping()}
        fallback={
          <main class="app-card">
            <section
              class="loading-section"
              aria-live="polite"
              aria-busy="true"
            >
              <div class="loading-spinner" aria-hidden="true"></div>
              <p class="loading-label">Loading</p>
            </section>
          </main>
        }
      >
        <main class="app-card">
          <header class="topbar">
            <button class="brand-button" onClick={returnToLanding}>
              <h1>SH2 Elders Quorum</h1>
            </button>
            <div class={`topbar-actions ${session() ? "session-actions" : ""}`}>
              <Show
                when={session()}
                fallback={
                  <>
                    <button
                      class="ghost-button"
                      onClick={() => setCurrentView("sign-in")}
                    >
                      Sign in
                    </button>
                    <button
                      class="primary-button"
                      onClick={() => setCurrentView("sign-up")}
                    >
                      Create account
                    </button>
                  </>
                }
              >
                <div class="notifications-anchor" ref={notificationAnchor}>
                  <button
                    class="ghost-button icon-button"
                    aria-label="Notifications"
                    aria-expanded={showNotifications()}
                    onClick={openNotifications}
                  >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M12 22a2.8 2.8 0 0 0 2.7-2h-5.4A2.8 2.8 0 0 0 12 22Zm7-6h-1V9a6 6 0 0 0-4.5-5.8V2h-3v1.2A6 6 0 0 0 6 9v7H5v2h14v-2Z" />
                    </svg>
                    <Show when={unreadCount() > 0}>
                      <span class="badge">{unreadCount()}</span>
                    </Show>
                  </button>
                  <Show when={showNotifications()}>
                    <NotificationPanel />
                  </Show>
                </div>
                <div class="desktop-account-actions">
                  <Show when={session()?.is_admin}>
                    <button
                      class="ghost-button"
                      onClick={() => {
                        setCurrentView(
                          currentView() === "admin" ? "home" : "admin",
                        );
                        setThread(null);
                        setShowNotifications(false);
                        setShowMobileAccountMenu(false);
                      }}
                    >
                      Admin
                    </button>
                  </Show>
                  <button class="ghost-button" onClick={openManageAccount}>
                    Manage account
                  </button>
                  <button class="ghost-button" onClick={signOut}>
                    Log out
                  </button>
                </div>
                <div class="mobile-account-menu" ref={mobileAccountMenuAnchor}>
                  <button
                    class="ghost-button icon-button menu-toggle"
                    type="button"
                    aria-label="Account menu"
                    aria-expanded={showMobileAccountMenu()}
                    onClick={() => {
                      setShowMobileAccountMenu(!showMobileAccountMenu());
                      setShowNotifications(false);
                    }}
                  >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                      <path d="M4 7h16v2H4V7Zm0 4h16v2H4v-2Zm0 4h16v2H4v-2Z" />
                    </svg>
                  </button>
                  <Show when={showMobileAccountMenu()}>
                    <div class="mobile-account-menu-panel">
                      <Show when={session()?.is_admin}>
                        <button
                          class="ghost-button"
                          onClick={() => {
                            setCurrentView(
                              currentView() === "admin" ? "home" : "admin",
                            );
                            setThread(null);
                            setShowNotifications(false);
                            setShowMobileAccountMenu(false);
                          }}
                        >
                          Admin
                        </button>
                      </Show>
                      <button class="ghost-button" onClick={openManageAccount}>
                        Manage account
                      </button>
                      <button class="ghost-button" onClick={signOut}>
                        Log out
                      </button>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>
          </header>

          <Show when={currentView() !== "home"}>
            <div class="back-row">
              <button
                class="back-button"
                type="button"
                aria-label="Back to landing"
                title="Back"
                onClick={returnToLanding}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M20 11H7.8l5.6-5.6L12 4 4 12l8 8 1.4-1.4L7.8 13H20v-2Z" />
                </svg>
              </button>
            </div>
          </Show>

          <Show when={error()}>
            <DismissibleMessage variant="error" onDismiss={() => setError("")}>
              {error()}
            </DismissibleMessage>
          </Show>
          <Show when={notice()}>
            <DismissibleMessage
              variant="success"
              onDismiss={() => setNotice("")}
            >
              {notice()}
            </DismissibleMessage>
          </Show>

          <Show when={!session()}>
            <Show when={currentView() === "sign-in"}>
              <SignInView />
            </Show>
            <Show when={currentView() === "sign-up"}>
              <SignUpView />
            </Show>
            <Show when={currentView() === "home"}>
              <LandingSection />
            </Show>
          </Show>

          <Show
            when={session() && sessionKind() === "password_change_required"}
          >
            <section class="content-panel">
              <h2>Change your password</h2>
              <p>
                A member of the EQ presidency reset your password. You must
                choose a new password before a full session can be issued.
              </p>
              <form class="stack-form narrow" onSubmit={submitChangePassword}>
                <input
                  type="password"
                  placeholder="Temporary password"
                  value={changePasswordForm().current_password}
                  onInput={(event) =>
                    setChangePasswordForm({
                      ...changePasswordForm(),
                      current_password: event.currentTarget.value,
                    })
                  }
                />
                <input
                  type="password"
                  placeholder="New password"
                  value={changePasswordForm().new_password}
                  onInput={(event) =>
                    setChangePasswordForm({
                      ...changePasswordForm(),
                      new_password: event.currentTarget.value,
                    })
                  }
                />
                <input
                  type="password"
                  placeholder="Confirm new password"
                  value={changePasswordForm().confirm_password}
                  onInput={(event) =>
                    setChangePasswordForm({
                      ...changePasswordForm(),
                      confirm_password: event.currentTarget.value,
                    })
                  }
                />
                <button class="primary-button" type="submit">
                  Change password
                </button>
              </form>
            </section>
          </Show>

          <Show when={session() && sessionKind() === "signed_up"}>
            <Show
              when={currentView() === "account"}
              fallback={
                <div class="content-stack">
                  <section class="warning-card">
                    <p>
                      A member of the EQ presidency will need to approve your
                      account before you can see or create posts
                    </p>
                  </section>
                  <section class="content-panel landing-content-panel">
                    <LandingSection />
                  </section>
                </div>
              }
            >
              <ManageAccountView />
            </Show>
          </Show>

          <Show when={session() && sessionKind() === "full"}>
            <div
              classList={{
                "dashboard-grid": true,
                "admin-view": currentView() === "admin",
              }}
            >
              <section class="column-main">
                <Show
                  when={currentView() === "admin"}
                  fallback={
                    <>
                      <Show
                        when={currentView() === "account"}
                        fallback={
                          <>
                            <AccountStatusWarning />
                            <LandingSection withFeedDivider />
                            <FeedSection />
                          </>
                        }
                      >
                        <ManageAccountView />
                      </Show>

                      <ThreadModal />
                      <PostModal />
                      <SurveyModal />
                    </>
                  }
                >
                  <AdminDashboard />
                </Show>
              </section>
            </div>
          </Show>
          <Show when={!session() || sessionKind() !== "full"}>
            <SurveyModal />
          </Show>
        </main>
      </Show>
    </div>
  );
}
