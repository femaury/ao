import bodyParser from 'body-parser'

import config from '../config.js'

import errors from './errors/errors.js'
import createLogger from '../domain/lib/logger.js'

import pouchDbClient from '../domain/clients/pouchdb.js'
import cuClient from '../domain/clients/cu.js'
import sequencerClient from '../domain/clients/sequencer.js'

import { initMsgsWith, processMsgWith, crankMsgsWith } from '../domain/lib/main.js'

const logger = createLogger('@permaweb/ao/servers/mu')

const dbInstance = pouchDbClient.pouchDb('ao-cache')

const SEQUENCER_URL = config.sequencerUrl

const initMsgs = initMsgsWith({
  selectNode: cuClient.selectNode,
  findLatestCacheTx: pouchDbClient.findLatestTxWith({ pouchDb: dbInstance }),
  cacheTx: pouchDbClient.saveTxWith({ pouchDb: dbInstance, logger }),
  findSequencerTx: sequencerClient.findTxWith({ SEQUENCER_URL }),
  writeSequencerTx: sequencerClient.writeInteractionWith({ SEQUENCER_URL }),
  fetchMsgs: cuClient.messages,
  saveMsg: pouchDbClient.saveMsgWith({ pouchDb: dbInstance, logger }),
  findLatestMsgs: pouchDbClient.findLatestMsgsWith({ pouchDb: dbInstance, logger }),
  logger
})

const processMsg = processMsgWith({
  selectNode: cuClient.selectNode,
  findLatestCacheTx: pouchDbClient.findLatestTxWith({ pouchDb: dbInstance }),
  cacheTx: pouchDbClient.saveTxWith({ pouchDb: dbInstance, logger }),
  findSequencerTx: sequencerClient.findTxWith({ SEQUENCER_URL }),
  writeSequencerTx: sequencerClient.writeInteractionWith({ SEQUENCER_URL }),
  buildAndSign: sequencerClient.buildAndSignWith(),
  fetchMsgs: cuClient.messages,
  saveMsg: pouchDbClient.saveMsgWith({ pouchDb: dbInstance, logger }),
  updateMsg: pouchDbClient.updateMsgWith({ pouchDb: dbInstance, logger }),
  findLatestMsgs: pouchDbClient.findLatestMsgsWith({ pouchDb: dbInstance, logger }),
  logger
})

const crankMsgs = crankMsgsWith({
  processMsg,
  logger
})

function injectDomain (req, _res, next) {
  req.domain = {}
  req.domain.initMsgs = initMsgs
  req.domain.crankMsgs = crankMsgs
  next()
}

const mountMiddlewares = (app) => [
  bodyParser.json(),
  bodyParser.urlencoded({ extended: false }),
  injectDomain,
  errors
].reduce(
  (app, middleware) => app.use(middleware),
  app
)

export default mountMiddlewares
